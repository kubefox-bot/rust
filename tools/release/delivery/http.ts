export const centerHttp = ({
  origin,
  ca,
}: Readonly<{ origin: string; ca: string }>) => {
  const request = async ({
    path,
    method = "GET",
    body,
    cookie,
  }: Readonly<{
    path: string;
    method?: "GET" | "POST";
    body?: unknown;
    cookie?: string;
  }>): Promise<Response> =>
    fetch(`${origin}${path}`, {
      method,
      redirect: "error",
      tls: { ca },
      headers: {
        Origin: origin,
        "X-HMail-Request": "1",
        "Content-Type": "application/json",
        ...(cookie ? { Cookie: cookie } : {}),
      },
      body: method === "POST" ? JSON.stringify(body ?? {}) : undefined,
    });
  return request;
};
