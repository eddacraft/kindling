/**
 * Type declarations for sql.js
 *
 * sql.js provides its own types but they're not always resolved correctly.
 * This module declaration ensures proper type inference.
 */

declare module 'sql.js' {
  export interface SqlJsStatic {
    Database: typeof Database;
  }

  export interface QueryExecResult {
    columns: string[];
    values: SqlValue[][];
  }

  export type SqlValue = string | number | Uint8Array | null;

  export interface ParamsObject {
    [key: string]: SqlValue;
  }

  export type ParamsCallback = (obj: ParamsObject) => void;
  export type BindParams = SqlValue[] | ParamsObject | null;

  export interface StatementIteratorResult {
    /** The resulting statement if there was no error */
    value?: Statement;
    /** True if we reached the end of the string */
    done: boolean;
  }

  export interface Statement {
    bind(params?: BindParams): boolean;
    step(): boolean;
    getAsObject(params?: ParamsObject): ParamsObject;
    getColumnNames(): string[];
    get(params?: BindParams): SqlValue[];
    run(params?: BindParams): void;
    reset(): void;
    free(): boolean;
  }

  export class Database {
    constructor(data?: ArrayLike<number> | Buffer | null);

    run(sql: string, params?: BindParams): Database;
    exec(sql: string, params?: BindParams): QueryExecResult[];
    each(sql: string, params: BindParams, callback: ParamsCallback, done: () => void): Database;
    each(sql: string, callback: ParamsCallback, done: () => void): Database;
    prepare(sql: string, params?: BindParams): Statement;
    iterateStatements(sql: string): StatementIteratorResult;
    export(): Uint8Array;
    close(): void;
    getRowsModified(): number;
    create_function(name: string, func: (...args: SqlValue[]) => SqlValue): Database;
    create_aggregate(
      name: string,
      functions: {
        init?: () => unknown;
        step: (state: unknown, ...values: SqlValue[]) => unknown;
        finalize: (state: unknown) => SqlValue;
      },
    ): Database;
  }

  export interface SqlJsConfig {
    locateFile?: (filename: string) => string;
    wasmBinary?: ArrayBuffer;
  }

  export default function initSqlJs(config?: SqlJsConfig): Promise<SqlJsStatic>;
}
