const DEFAULT_BUILD = "development";
const MAX_BUILD_LENGTH = 128;
export const publicBuild = (): Readonly<{ version: string }> => {
  const version: unknown = import.meta.env.PUBLIC_BUILD_VERSION;
  if (version === undefined) return { version: DEFAULT_BUILD };
  if (
    typeof version !== "string" ||
    version.length === 0 ||
    version.length > MAX_BUILD_LENGTH ||
    /[\u0000-\u001f]/.test(version)
  ) {
    throw new Error("Invalid public build version");
  }
  return { version };
};
