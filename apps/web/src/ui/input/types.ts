import type {
  HTMLInputAttributes,
  HTMLInputTypeAttribute,
} from "svelte/elements";

export type InputValue = string | number | undefined;
export type InputType = Exclude<
  HTMLInputTypeAttribute,
  | "checkbox"
  | "radio"
  | "file"
  | "button"
  | "submit"
  | "reset"
  | "image"
  | "hidden"
>;
export type InputProps<Value extends InputValue> = Omit<
  HTMLInputAttributes,
  | "value"
  | "defaultValue"
  | "defaultvalue"
  | "type"
  | "children"
  | "checked"
  | "defaultChecked"
  | "defaultchecked"
  | "group"
  | "files"
> & {
  value?: Value;
  type?: InputType;
  element?: HTMLInputElement;
};
