import { CENTER_HEALTH_TEST } from "./conts";

export const requireDirectHealthcheck = ({
  value,
  expectedBuild,
}: Readonly<{ value: unknown; expectedBuild?: string }>): void => {
  if (
    typeof value !== "object" ||
    value === null ||
    !("Test" in value) ||
    !Array.isArray(value.Test)
  )
    throw new Error("Runtime image requires a direct healthcheck");
  const test: readonly unknown[] = value.Test;
  const expected =
    expectedBuild === undefined
      ? CENTER_HEALTH_TEST
      : [...CENTER_HEALTH_TEST, "--expect-build", expectedBuild];
  if (
    test.length !== expected.length ||
    !test.every((argument, index) => argument === expected[index])
  )
    throw new Error(
      "Runtime image requires a direct healthcheck with the center health argv",
    );
};
