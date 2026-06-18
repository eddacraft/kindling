//! Integration tests for `KindlingService`.
//!
//! Behavioural parity with `KindlingService` in
//! `packages/kindling-core/src/service/kindling-service.ts`. Exact-ID parity
//! with the TS service is impossible (both sides mint random UUIDs for
//! defaulted ids), so time-sensitive and identity-sensitive assertions pass
//! explicit ids / timestamps through the deterministic `*_at` seams.

use kindling_service::{
    AppendObservationOptions, CloseCapsuleOptions, CreatePinOptions, KindlingService,
    OpenCapsuleOptions, ServiceError,
};
use kindling_types::{
    CapsuleStatus, CapsuleType, ObservationInput, ObservationKind, PinTargetType, RetrieveOptions,
    RetrievedEntity, ScopeIds,
};

fn service() -> KindlingService {
    KindlingService::open_in_memory().expect("open in-memory service")
}

fn session_scope(session: &str) -> ScopeIds {
    ScopeIds {
        session_id: Some(session.to_string()),
        ..Default::default()
    }
}

fn obs_input(content: &str, scope: ScopeIds) -> ObservationInput {
    ObservationInput {
        id: None,
        kind: ObservationKind::Message,
        content: content.to_string(),
        provenance: None,
        ts: None,
        scope_ids: scope,
        redacted: None,
    }
}

// ===== open_capsule =====

#[test]
fn open_capsule_happy_path() {
    let svc = service();
    let capsule = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::Session,
            intent: "do the thing".to_string(),
            scope_ids: session_scope("s1"),
            id: None,
        })
        .expect("open capsule");

    assert_eq!(capsule.status, CapsuleStatus::Open);
    assert_eq!(capsule.intent, "do the thing");
    assert!(capsule.observation_ids.is_empty());
    assert!(capsule.closed_at.is_none());
    // Defaulted id is a bare UUID (no prefix).
    assert!(!capsule.id.is_empty());
    assert!(!capsule.id.contains('_'));

    // Persisted and retrievable.
    let fetched = svc.get_capsule(&capsule.id).expect("get").expect("present");
    assert_eq!(fetched.id, capsule.id);
}

#[test]
fn open_capsule_duplicate_session_conflict() {
    let svc = service();
    svc.open_capsule(OpenCapsuleOptions {
        kind: CapsuleType::Session,
        intent: "first".to_string(),
        scope_ids: session_scope("dup"),
        id: None,
    })
    .expect("first open");

    let err = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::Session,
            intent: "second".to_string(),
            scope_ids: session_scope("dup"),
            id: None,
        })
        .expect_err("duplicate must conflict");

    assert!(matches!(err, ServiceError::Conflict(_)), "got {err:?}");
}

#[test]
fn open_capsule_non_session_allows_no_session_scope() {
    let svc = service();
    let capsule = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::PocketflowNode,
            intent: "node run".to_string(),
            scope_ids: ScopeIds::default(),
            id: Some("explicit-id".to_string()),
        })
        .expect("open node capsule");
    assert_eq!(capsule.id, "explicit-id");
    assert_eq!(capsule.kind, CapsuleType::PocketflowNode);
}

#[test]
fn open_capsule_empty_intent_is_validation_error() {
    let svc = service();
    let err = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::Session,
            intent: "   ".to_string(),
            scope_ids: session_scope("s2"),
            id: None,
        })
        .expect_err("empty intent rejected");
    assert!(matches!(err, ServiceError::Validation(_)), "got {err:?}");
}

// ===== close_capsule =====

#[test]
fn close_capsule_not_found() {
    let svc = service();
    let err = svc
        .close_capsule("nope", CloseCapsuleOptions::default())
        .expect_err("missing capsule");
    assert!(matches!(err, ServiceError::NotFound(_)), "got {err:?}");
}

#[test]
fn close_capsule_already_closed() {
    let svc = service();
    let capsule = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::Session,
            intent: "to close".to_string(),
            scope_ids: session_scope("close1"),
            id: None,
        })
        .expect("open");

    svc.close_capsule(&capsule.id, CloseCapsuleOptions::default())
        .expect("first close");

    let err = svc
        .close_capsule(&capsule.id, CloseCapsuleOptions::default())
        .expect_err("second close");
    assert!(matches!(err, ServiceError::AlreadyClosed(_)), "got {err:?}");
}

#[test]
fn close_capsule_sets_status_and_closed_at() {
    let svc = service();
    let capsule = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::Session,
            intent: "closing".to_string(),
            scope_ids: session_scope("close2"),
            id: None,
        })
        .expect("open");

    let closed = svc
        .close_capsule_at(
            &capsule.id,
            CloseCapsuleOptions::default(),
            1_700_000_000_000,
        )
        .expect("close");
    assert_eq!(closed.status, CapsuleStatus::Closed);
    assert_eq!(closed.closed_at, Some(1_700_000_000_000));
}

#[test]
fn close_capsule_generate_summary_persists_prefixed_summary() {
    let svc = service();
    let capsule = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::Session,
            intent: "summarise".to_string(),
            scope_ids: session_scope("sum1"),
            id: None,
        })
        .expect("open");

    let closed = svc
        .close_capsule_at(
            &capsule.id,
            CloseCapsuleOptions {
                generate_summary: true,
                summary_content: Some("the work was done".to_string()),
                confidence: None,
            },
            1_700_000_000_000,
        )
        .expect("close with summary");
    assert_eq!(closed.status, CapsuleStatus::Closed);

    let summary = svc
        .get_latest_summary(&capsule.id)
        .expect("get summary")
        .expect("summary present");
    assert!(summary.id.starts_with("sum_"), "id was {}", summary.id);
    assert_eq!(summary.confidence, 1.0);
    assert_eq!(summary.capsule_id, capsule.id);
    assert_eq!(summary.content, "the work was done");
    assert!(summary.evidence_refs.is_empty());
    assert_eq!(summary.created_at, 1_700_000_000_000);
}

#[test]
fn close_capsule_generate_summary_without_content_skips_summary() {
    let svc = service();
    let capsule = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::Session,
            intent: "no summary".to_string(),
            scope_ids: session_scope("sum2"),
            id: None,
        })
        .expect("open");

    svc.close_capsule(
        &capsule.id,
        CloseCapsuleOptions {
            generate_summary: true,
            summary_content: None,
            confidence: None,
        },
    )
    .expect("close");

    assert!(svc
        .get_latest_summary(&capsule.id)
        .expect("get summary")
        .is_none());
}

#[test]
fn close_capsule_invalid_confidence_is_validation_error() {
    let svc = service();
    let capsule = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::Session,
            intent: "bad conf".to_string(),
            scope_ids: session_scope("conf1"),
            id: None,
        })
        .expect("open");

    let err = svc
        .close_capsule(
            &capsule.id,
            CloseCapsuleOptions {
                generate_summary: true,
                summary_content: Some("x".to_string()),
                confidence: Some(2.0),
            },
        )
        .expect_err("confidence out of range");
    assert!(matches!(err, ServiceError::Validation(_)), "got {err:?}");
}

// ===== append_observation =====

#[test]
fn append_observation_happy_path_is_retrievable() {
    let svc = service();
    let scope = session_scope("a1");
    let obs = svc
        .append_observation(
            obs_input("the quick brown fox", scope.clone()),
            AppendObservationOptions::default(),
        )
        .expect("append");

    assert!(!obs.id.is_empty());
    assert!(!obs.id.contains('_')); // bare UUID
    assert!(!obs.redacted);

    let result = svc
        .retrieve(RetrieveOptions {
            query: "fox".to_string(),
            scope_ids: scope,
            token_budget: None,
            max_candidates: None,
            include_redacted: None,
        })
        .expect("retrieve");
    assert!(result.candidates.iter().any(|c| matches!(
        &c.entity,
        RetrievedEntity::Observation(o) if o.id == obs.id
    )));
}

#[test]
fn append_observation_attaches_to_capsule() {
    let svc = service();
    let scope = session_scope("a2");
    let capsule = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::Session,
            intent: "attach".to_string(),
            scope_ids: scope.clone(),
            id: None,
        })
        .expect("open");

    let obs = svc
        .append_observation(
            obs_input("attached content", scope),
            AppendObservationOptions {
                capsule_id: Some(capsule.id.clone()),
                validate: true,
            },
        )
        .expect("append");

    let fetched = svc.get_capsule(&capsule.id).expect("get").expect("present");
    assert_eq!(fetched.observation_ids, vec![obs.id]);
}

#[test]
fn append_observation_empty_content_is_validation_error() {
    let svc = service();
    let err = svc
        .append_observation(
            obs_input("   ", session_scope("a3")),
            AppendObservationOptions::default(),
        )
        .expect_err("whitespace content rejected");
    assert!(matches!(err, ServiceError::Validation(_)), "got {err:?}");
}

#[test]
fn append_observation_validate_false_skips_validation() {
    let svc = service();
    // Whitespace content would normally fail validation; with validate:false
    // it is stored as-is (after secret masking, which leaves it untouched).
    let obs = svc
        .append_observation(
            obs_input("   ", session_scope("a4")),
            AppendObservationOptions {
                capsule_id: None,
                validate: false,
            },
        )
        .expect("append without validation");
    let stored = svc.get_observation(&obs.id).expect("get").expect("present");
    assert_eq!(stored.content, "   ");
}

#[test]
fn append_observation_masks_secrets_at_service_boundary() {
    let svc = service();
    // Pattern + expected mask come straight from the kindling-filter fixtures.
    let raw = "api_key=abcdef123456789 and more text";
    let expected = kindling_filter::mask_secrets(raw);
    assert_eq!(expected, "api_key=[REDACTED] and more text");

    let obs = svc
        .append_observation(
            obs_input(raw, session_scope("sec1")),
            AppendObservationOptions::default(),
        )
        .expect("append");

    // Returned observation already carries the masked content.
    assert_eq!(obs.content, expected);

    let stored = svc.get_observation(&obs.id).expect("get").expect("present");
    assert_eq!(stored.content, expected);
    assert!(!stored.content.contains("abcdef123456789"));
}

#[test]
fn append_observation_masks_anthropic_key() {
    let svc = service();
    let raw = "using sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWxYz1234 for auth";
    let obs = svc
        .append_observation(
            obs_input(raw, session_scope("sec2")),
            AppendObservationOptions::default(),
        )
        .expect("append");
    assert!(!obs
        .content
        .contains("sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWxYz1234"));
    assert_eq!(obs.content, kindling_filter::mask_secrets(raw));
}

// ===== pin / unpin / list_pins =====

#[test]
fn pin_has_prefix_and_ttl_expiry() {
    let svc = service();
    let scope = session_scope("p1");
    let pin = svc
        .pin_at(
            CreatePinOptions {
                target_type: PinTargetType::Observation,
                target_id: "obs-123".to_string(),
                note: Some("keep this".to_string()),
                ttl_ms: Some(1000),
                scope_ids: Some(scope.clone()),
            },
            5_000,
        )
        .expect("pin");

    assert!(pin.id.starts_with("pin_"), "id was {}", pin.id);
    assert_eq!(pin.created_at, 5_000);
    assert_eq!(pin.expires_at, Some(6_000));
    assert_eq!(pin.reason.as_deref(), Some("keep this"));

    // Active just before expiry.
    let active = svc.list_pins_at(Some(&scope), 5_999).expect("list active");
    assert!(active.iter().any(|p| p.id == pin.id));

    // Gone at/after expiry.
    let expired = svc.list_pins_at(Some(&scope), 6_000).expect("list expired");
    assert!(!expired.iter().any(|p| p.id == pin.id));
}

#[test]
fn pin_without_ttl_never_expires() {
    let svc = service();
    let scope = session_scope("p2");
    let pin = svc
        .pin_at(
            CreatePinOptions {
                target_type: PinTargetType::Summary,
                target_id: "sum-1".to_string(),
                note: None,
                ttl_ms: None,
                scope_ids: Some(scope.clone()),
            },
            10,
        )
        .expect("pin");
    assert!(pin.expires_at.is_none());
    let active = svc
        .list_pins_at(Some(&scope), i64::MAX)
        .expect("list far future");
    assert!(active.iter().any(|p| p.id == pin.id));
}

#[test]
fn unpin_removes_pin() {
    let svc = service();
    let scope = session_scope("p3");
    let pin = svc
        .pin_at(
            CreatePinOptions {
                target_type: PinTargetType::Observation,
                target_id: "obs-x".to_string(),
                note: None,
                ttl_ms: None,
                scope_ids: Some(scope.clone()),
            },
            0,
        )
        .expect("pin");

    svc.unpin(&pin.id).expect("unpin");
    let pins = svc.list_pins_at(Some(&scope), 0).expect("list");
    assert!(!pins.iter().any(|p| p.id == pin.id));

    // Removing again is a NotFound error (store contract).
    let err = svc.unpin(&pin.id).expect_err("double unpin");
    assert!(matches!(err, ServiceError::Store(_)), "got {err:?}");
}

// ===== retrieve: pinned observation surfaces =====

#[test]
fn retrieve_surfaces_pinned_observation() {
    let svc = service();
    let scope = session_scope("r1");
    let obs = svc
        .append_observation(
            obs_input("pinned material here", scope.clone()),
            AppendObservationOptions::default(),
        )
        .expect("append");

    svc.pin_at(
        CreatePinOptions {
            target_type: PinTargetType::Observation,
            target_id: obs.id.clone(),
            note: None,
            ttl_ms: None,
            scope_ids: Some(scope.clone()),
        },
        1_000,
    )
    .expect("pin");

    let result = svc
        .retrieve_at(
            RetrieveOptions {
                query: "nomatch".to_string(),
                scope_ids: scope,
                token_budget: None,
                max_candidates: None,
                include_redacted: None,
            },
            2_000,
        )
        .expect("retrieve");

    assert!(result.pins.iter().any(|p| matches!(
        &p.target,
        RetrievedEntity::Observation(o) if o.id == obs.id
    )));
}

// ===== forget =====

#[test]
fn forget_redacts_observation() {
    let svc = service();
    let obs = svc
        .append_observation(
            obs_input("sensitive note", session_scope("f1")),
            AppendObservationOptions::default(),
        )
        .expect("append");

    svc.forget(&obs.id).expect("forget");

    let stored = svc.get_observation(&obs.id).expect("get").expect("present");
    assert!(stored.redacted);
    assert_eq!(stored.content, "[redacted]");
}

#[test]
fn forget_missing_observation_errors() {
    let svc = service();
    let err = svc.forget("ghost").expect_err("missing");
    assert!(matches!(err, ServiceError::Store(_)), "got {err:?}");
}

// ===== read accessors =====

#[test]
fn get_open_capsule_for_session() {
    let svc = service();
    let capsule = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::Session,
            intent: "live".to_string(),
            scope_ids: session_scope("open1"),
            id: None,
        })
        .expect("open");

    let found = svc
        .get_open_capsule("open1")
        .expect("get open")
        .expect("present");
    assert_eq!(found.id, capsule.id);

    assert!(svc
        .get_open_capsule("does-not-exist")
        .expect("get open")
        .is_none());
}

#[test]
fn get_summary_by_id() {
    let svc = service();
    let capsule = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::Session,
            intent: "s".to_string(),
            scope_ids: session_scope("gs1"),
            id: None,
        })
        .expect("open");
    svc.close_capsule_at(
        &capsule.id,
        CloseCapsuleOptions {
            generate_summary: true,
            summary_content: Some("done".to_string()),
            confidence: Some(0.5),
        },
        1,
    )
    .expect("close");

    let latest = svc
        .get_latest_summary(&capsule.id)
        .expect("latest")
        .expect("present");
    let by_id = svc
        .get_summary(&latest.id)
        .expect("by id")
        .expect("present");
    assert_eq!(by_id.id, latest.id);
    assert_eq!(by_id.confidence, 0.5);
}

// ===== injection context =====

fn repo_scope(repo: &str) -> ScopeIds {
    ScopeIds {
        repo_id: Some(repo.to_string()),
        ..Default::default()
    }
}

#[test]
fn session_start_context_orders_recent_and_resolves_pins() {
    let svc = service();
    let scope = repo_scope("/r");

    // Three observations at increasing ts; newest must come first.
    let mut obs_ids = Vec::new();
    for (i, ts) in [(1, 1000_i64), (2, 2000), (3, 3000)] {
        let o = svc
            .append_observation_at(
                obs_input(&format!("obs {i}"), scope.clone()),
                AppendObservationOptions::default(),
                ts,
            )
            .expect("append");
        obs_ids.push(o.id);
    }

    // Pin the middle observation with a note.
    svc.pin_at(
        CreatePinOptions {
            target_type: PinTargetType::Observation,
            target_id: obs_ids[1].clone(),
            note: Some("keep this".to_string()),
            ttl_ms: None,
            scope_ids: Some(scope.clone()),
        },
        3000,
    )
    .expect("pin");

    let ctx = svc
        .session_start_context_at(&scope, 10, 3000)
        .expect("session start context");

    // Recent: newest first.
    let recent: Vec<&str> = ctx.recent.iter().map(|o| o.content.as_str()).collect();
    assert_eq!(recent, vec!["obs 3", "obs 2", "obs 1"]);

    // Pin resolved to target content + note.
    assert_eq!(ctx.pins.len(), 1);
    assert_eq!(ctx.pins[0].note.as_deref(), Some("keep this"));
    assert_eq!(ctx.pins[0].content.as_deref(), Some("obs 2"));
}

#[test]
fn session_start_context_respects_max_results_and_redaction() {
    let svc = service();
    let scope = repo_scope("/r");

    let mut ids = Vec::new();
    for (i, ts) in [(1, 1000_i64), (2, 2000), (3, 3000)] {
        let o = svc
            .append_observation_at(
                obs_input(&format!("obs {i}"), scope.clone()),
                AppendObservationOptions::default(),
                ts,
            )
            .expect("append");
        ids.push(o.id);
    }
    // Redact the newest — it must drop out.
    svc.forget(&ids[2]).expect("forget");

    let ctx = svc.session_start_context_at(&scope, 1, 4000).expect("ctx");
    // Cap of 1, redacted excluded → only "obs 2".
    assert_eq!(ctx.recent.len(), 1);
    assert_eq!(ctx.recent[0].content, "obs 2");
}

#[test]
fn session_start_context_excludes_expired_pins() {
    let svc = service();
    let scope = repo_scope("/r");
    let o = svc
        .append_observation_at(
            obs_input("target", scope.clone()),
            AppendObservationOptions::default(),
            1000,
        )
        .expect("append");
    // Pin expires at 1000 + 500 = 1500.
    svc.pin_at(
        CreatePinOptions {
            target_type: PinTargetType::Observation,
            target_id: o.id,
            note: None,
            ttl_ms: Some(500),
            scope_ids: Some(scope.clone()),
        },
        1000,
    )
    .expect("pin");

    // At now=2000 the pin has expired.
    let ctx = svc.session_start_context_at(&scope, 10, 2000).expect("ctx");
    assert!(ctx.pins.is_empty(), "expired pin must not appear");
}

#[test]
fn session_start_context_is_empty_when_nothing() {
    let svc = service();
    let ctx = svc
        .session_start_context_at(&repo_scope("/empty"), 10, 1000)
        .expect("ctx");
    assert!(ctx.is_empty());
}

#[test]
fn pre_compact_context_picks_latest_summary_and_resolves_pins() {
    let svc = service();
    let scope = repo_scope("/r");

    // Two capsules in the repo, each with a summary; newest summary wins.
    let cap1 = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::PocketflowNode,
            intent: "c1".to_string(),
            scope_ids: scope.clone(),
            id: None,
        })
        .expect("open c1");
    svc.close_capsule_at(
        &cap1.id,
        CloseCapsuleOptions {
            generate_summary: true,
            summary_content: Some("older summary".to_string()),
            confidence: Some(0.7),
        },
        1000,
    )
    .expect("close c1");

    let cap2 = svc
        .open_capsule(OpenCapsuleOptions {
            kind: CapsuleType::PocketflowNode,
            intent: "c2".to_string(),
            scope_ids: scope.clone(),
            id: None,
        })
        .expect("open c2");
    svc.close_capsule_at(
        &cap2.id,
        CloseCapsuleOptions {
            generate_summary: true,
            summary_content: Some("newer summary".to_string()),
            confidence: Some(0.9),
        },
        2000,
    )
    .expect("close c2");

    // Pin a summary by id.
    let latest = svc
        .get_latest_summary(&cap2.id)
        .expect("get")
        .expect("present");
    svc.pin_at(
        CreatePinOptions {
            target_type: PinTargetType::Summary,
            target_id: latest.id.clone(),
            note: Some("summary pin".to_string()),
            ttl_ms: None,
            scope_ids: Some(scope.clone()),
        },
        2000,
    )
    .expect("pin");

    let ctx = svc
        .pre_compact_context_at(&scope, 2000)
        .expect("pre compact context");

    let summary = ctx.latest_summary.expect("a summary");
    assert_eq!(summary.content, "newer summary");

    assert_eq!(ctx.pins.len(), 1);
    assert_eq!(ctx.pins[0].note.as_deref(), Some("summary pin"));
    assert_eq!(ctx.pins[0].content.as_deref(), Some("newer summary"));
}

#[test]
fn pre_compact_context_is_empty_when_nothing() {
    let svc = service();
    let ctx = svc
        .pre_compact_context_at(&repo_scope("/empty"), 1000)
        .expect("ctx");
    assert!(ctx.is_empty());
    assert!(ctx.latest_summary.is_none());
}
