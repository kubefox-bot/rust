import type { DeliveryDescriptor } from "../delivery-descriptor/types";
export type PublishedRelease = Readonly<{
  sourceSha: string;
  sequence: number;
  imageDigest: string;
  releaseId: number;
}>;
export type PublicationPorts = Readonly<{
  copyImage: () => Promise<string>;
  createDraft: () => Promise<
    Readonly<{ id: number; published: boolean; descriptor?: string }>
  >;
  sign: (path: string) => Promise<string>;
  upload: (
    releaseId: number,
    files: ReadonlyArray<Readonly<{ name: string; path: string }>>,
  ) => Promise<void>;
  verifyAssets: (releaseId: number, names: readonly string[]) => Promise<void>;
}>;
export type PromotionPorts = Readonly<{
  currentHead: () => Promise<string>;
  publishedDescriptors: () => Promise<readonly DeliveryDescriptor[]>;
  publishDraft: (releaseId: number) => Promise<void>;
}>;
export type PublishInput = Readonly<{
  directory: string;
  sourceSha: string;
  sequence: number;
  ports: PublicationPorts;
}>;
export type PromotionInput = Readonly<{
  release: PublishedRelease;
  descriptor: DeliveryDescriptor;
  ports: PromotionPorts;
}>;
export type PublicationConfig = Readonly<{
  root: string;
  directory: string;
  sourceSha: string;
  sequence: number;
  token: string;
  registryAuthFile: string;
  trustedRoot: string;
  cosign: string;
  mode: "publish" | "promote";
}>;
export type HttpTransport = (
  input: string | URL,
  init?: RequestInit,
) => Promise<Response>;
