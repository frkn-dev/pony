export interface FullError extends Error {
    code?: string;
    errno?: number;
    address?: string;
}
export declare function isFullError(err: unknown): err is FullError;
