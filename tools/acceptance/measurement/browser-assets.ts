export const additionalScripts = (
  input: Readonly<{ initial: readonly string[]; selected: readonly string[] }>,
): readonly string[] => {
  const initial = new Set(input.initial);
  return [...new Set(input.selected)].filter((path) => !initial.has(path));
};
