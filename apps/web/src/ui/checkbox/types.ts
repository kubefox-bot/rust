import type { HTMLInputAttributes } from "svelte/elements";

export type CheckboxProps = Omit<
  HTMLInputAttributes,
  | "type"
  | "children"
  | "checked"
  | "defaultChecked"
  | "defaultchecked"
  | "group"
  | "files"
> & {
  label: string;
  description?: string;
  checked?: boolean;
  tone?: "default" | "danger";
};
