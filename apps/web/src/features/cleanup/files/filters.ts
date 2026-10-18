import type { FileFilters, FilesQuery } from "./types";

// Picker values are browser wall time. Keep fractional seconds beyond Date's milliseconds.
const utcBound = (value: string): string | undefined => {
  if (!value) return undefined;
  const date = new Date(value);
  if (Number.isNaN(date.getTime()))
    throw new Error("Выберите корректную дату и время.");
  const fraction = value.split(".")[1];
  return `${date.toISOString().split(".")[0]}.${(fraction ?? "").padEnd(9, "0")}Z`;
};

export const resolveFilters = (
  value: FileFilters,
): Readonly<{
  error: string;
  filters: Pick<FilesQuery, "createdFrom" | "createdTo" | "orphan">;
}> => {
  try {
    const from = utcBound(value.from);
    const to = utcBound(value.to);
    if (from && to && from > to)
      return { error: "Дата «с» должна быть не позже даты «по».", filters: {} };
    return {
      error: "",
      filters: {
        ...(from ? { createdFrom: from } : {}),
        ...(to ? { createdTo: to } : {}),
        ...(value.relation === "all"
          ? {}
          : { orphan: value.relation === "orphan" }),
      },
    };
  } catch (cause) {
    return {
      error: cause instanceof Error ? cause.message : "Некорректная дата.",
      filters: {},
    };
  }
};
