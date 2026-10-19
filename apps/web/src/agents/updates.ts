// Packaged builds use major.minor.patch+revision.unixTimestamp.
// Unknown versions do not prove that a newer package is available.
const parseBuild = (build: string) => {
  const match = /^(\d+)\.(\d+)\.(\d+)(?:\+([\da-f]+)\.(\d+))?$/.exec(build);
  return match
    ? {
        release: match.slice(1, 4).map(BigInt),
        timestamp: match[5] ? BigInt(match[5]) : null,
      }
    : null;
};

export const isNewerAgentBuild = (
  input: Readonly<{ current: string; available: string }>,
): boolean => {
  const current = parseBuild(input.current);
  const available = parseBuild(input.available);
  if (!current || !available) return false;
  for (const [index, value] of available.release.entries()) {
    const installed = current.release[index];
    if (installed === undefined) return false;
    if (value !== installed) return value > installed;
  }
  return (
    current.timestamp !== null &&
    available.timestamp !== null &&
    available.timestamp > current.timestamp
  );
};
