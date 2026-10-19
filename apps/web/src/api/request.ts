import { error as errorSchema } from "./contracts";
import {
  API_PREFIX,
  MAX_RESPONSE_BYTES,
  REQUEST_TIMEOUT_MS,
  ERROR_MESSAGES,
  UNSAFE_HEADERS,
} from "./conts";
import { decode, ContractError } from "./decode";
import type { RequestOptions } from "./types";
export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly correlationId?: string,
  ) {
    super(
      ERROR_MESSAGES[status] ??
        "Не удалось выполнить запрос. Проверьте состояние перед повторным действием.",
    );
  }
}
const responseValue = async (response: Response): Promise<unknown> => {
  if (!response.body) throw new Error("Сервер вернул пустой ответ.");
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > MAX_RESPONSE_BYTES)
        throw new Error("Ответ сервера превышает допустимый размер.");
      chunks.push(value);
    }
  } catch (error) {
    try {
      await reader.cancel();
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        "Не удалось прочитать и завершить ответ сервера.",
      );
    }
    throw error;
  } finally {
    reader.releaseLock();
  }
  const bytes = new Uint8Array(size);
  let cursor = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, cursor);
    cursor += chunk.byteLength;
  }
  try {
    const value: unknown = JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(bytes),
    );
    return value;
  } catch {
    throw new ContractError();
  }
};
const fetchApi = async (
  options: Readonly<{
    path: string;
    method?: RequestOptions<unknown>["method"];
    body?: unknown;
    signal?: AbortSignal;
  }>,
): Promise<Response> => {
  if (
    !options.path.startsWith("/") ||
    options.path.startsWith("//") ||
    /[?#\\]/.test(options.path.split("?")[0] ?? "") ||
    options.path.includes("..")
  )
    throw new Error("Invalid API path");
  const method = options.method ?? "GET";
  const signal = options.signal
    ? AbortSignal.any([options.signal, AbortSignal.timeout(REQUEST_TIMEOUT_MS)])
    : AbortSignal.timeout(REQUEST_TIMEOUT_MS);
  const headers = new Headers(method === "GET" ? undefined : UNSAFE_HEADERS);
  return fetch(`${API_PREFIX}${options.path}`, {
    method,
    headers,
    credentials: "same-origin",
    cache: "no-store",
    redirect: "error",
    signal,
    body: method === "GET" ? undefined : JSON.stringify(options.body ?? {}),
  });
};
const correlationId = (body: unknown): string | undefined => {
  try {
    return decode(errorSchema, body).correlation_id;
  } catch {
    return undefined;
  }
};
const checkResponse = async (response: Response): Promise<void> => {
  if (response.ok) return;
  // An invalid error document must not turn an HTTP failure into apparent success.
  const body = await responseValue(response).catch(() => undefined);
  throw new ApiError(response.status, correlationId(body));
};
export const requestJson = async <Value>(
  options: RequestOptions<Value>,
): Promise<Value> => {
  const response = await fetchApi(options);
  await checkResponse(response);
  return decode(options.responseSchema, await responseValue(response));
};
export const requestEmpty = async (
  options: Readonly<{
    path: string;
    method: "POST" | "PATCH" | "PUT";
    body?: unknown;
    signal?: AbortSignal;
  }>,
): Promise<void> => {
  await checkResponse(await fetchApi(options));
};
