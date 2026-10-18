export type KeyAction =
  | Readonly<{ kind: "none" | "close" }>
  | Readonly<{ kind: "focus"; index: number }>
  | Readonly<{ kind: "commit"; index: number; preventDefault: boolean }>;

export type KeyInput = Readonly<{
  key: string;
  open: boolean;
  activeIndex: number;
  selectedIndex: number;
  count: number;
}>;

export type MenuPlacement = Readonly<{
  side: "above" | "below";
  maxHeight: number;
}>;
export type SelectOption<Value extends string | number> = Readonly<{
  value: Value;
  label: string;
  description?: string;
}>;
