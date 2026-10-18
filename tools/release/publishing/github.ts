import {
  API_TIMEOUT_MS,
  MAX_API_BYTES,
  UPLOAD_TIMEOUT_MS,
  GITHUB_API_VERSION,
  GITHUB_ASSET_HOSTS,
} from "./conts";
import { GITHUB_REPOSITORY } from "../delivery-descriptor/conts";
import type { HttpTransport, PublicationConfig } from "./types";

export const githubClient = (
  config: PublicationConfig,
  transport: HttpTransport = fetch,
) => {
  const request = async ({
    path,
    method = "GET",
    body,
    accept = "application/vnd.github+json",
  }: Readonly<{
    path: string;
    method?: "GET" | "POST" | "PATCH" | "DELETE";
    body?: unknown;
    accept?: string;
  }>): Promise<Response> => {
    const response = await transport(
      `https://api.github.com/repos/${GITHUB_REPOSITORY}${path}`,
      {
        method,
        headers: {
          Authorization: `Bearer ${config.token}`,
          Accept: accept,
          "X-GitHub-Api-Version": GITHUB_API_VERSION,
          ...(body === undefined ? {} : { "Content-Type": "application/json" }),
        },
        body: body === undefined ? undefined : JSON.stringify(body),
        redirect: "manual",
        signal: AbortSignal.timeout(API_TIMEOUT_MS),
      },
    );
    if (accept === "application/octet-stream" && response.status === 302) {
      const location = response.headers.get("location");
      if (!location) throw new Error("GitHub asset redirect lacks destination");
      const destination = new URL(location);
      if (
        destination.protocol !== "https:" ||
        destination.username ||
        destination.password ||
        !GITHUB_ASSET_HOSTS.some((host) => host === destination.hostname)
      )
        throw new Error("Unexpected GitHub asset redirect");
      await response.body?.cancel();
      return transport(destination, {
        redirect: "error",
        signal: AbortSignal.timeout(API_TIMEOUT_MS),
      });
    }
    if (!response.ok && response.status !== 404)
      throw new Error(`GitHub request failed with HTTP ${response.status}`);
    return response;
  };
  const text = async (response: Response): Promise<string> => {
    if (!response.ok)
      throw new Error(`GitHub asset unavailable (${response.status})`);
    const chunks: Uint8Array[] = [];
    let size = 0;
    if (!response.body) throw new Error("GitHub response is empty");
    const reader = response.body.getReader();
    try {
      for (;;) {
        const item = await reader.read();
        if (item.done) break;
        size += item.value.byteLength;
        if (size > MAX_API_BYTES)
          throw new Error("GitHub metadata exceeds limit");
        chunks.push(item.value);
      }
    } finally {
      await reader.cancel();
      reader.releaseLock();
    }
    return Buffer.concat(chunks).toString("utf8");
  };
  const json = async (
    options: Parameters<typeof request>[0],
  ): Promise<unknown> => JSON.parse(await text(await request(options)));
  const upload = async ({
    releaseId,
    name,
    path,
  }: Readonly<{
    releaseId: number;
    name: string;
    path: string;
  }>): Promise<void> => {
    const file = Bun.file(path);
    if (!(await file.exists()) || file.size === 0)
      throw new Error("Missing release asset");
    const response = await transport(
      `https://uploads.github.com/repos/${GITHUB_REPOSITORY}/releases/${releaseId}/assets?name=${encodeURIComponent(name)}`,
      {
        method: "POST",
        headers: {
          Authorization: `Bearer ${config.token}`,
          "Content-Type": "application/octet-stream",
          "Content-Length": String(file.size),
          "X-GitHub-Api-Version": GITHUB_API_VERSION,
        },
        body: file,
        redirect: "error",
        signal: AbortSignal.timeout(UPLOAD_TIMEOUT_MS),
      },
    );
    if (!response.ok)
      throw new Error(`GitHub upload failed (${response.status})`);
    await response.body?.cancel();
  };
  return { request, json, text, upload };
};
export type GithubClient = ReturnType<typeof githubClient>;
