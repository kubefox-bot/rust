import type {
  HTMLAnchorAttributes,
  HTMLButtonAttributes,
} from "svelte/elements";

export type ButtonVariant =
  "primary" | "secondary" | "success" | "warning" | "danger" | "quiet" | "link";
export type ButtonSize = "regular" | "small";
type Appearance = Readonly<{ variant?: ButtonVariant; size?: ButtonSize }>;

export type ButtonProps = HTMLButtonAttributes &
  Appearance & {
    loading?: boolean;
    element?: HTMLButtonElement;
  };
export type ButtonLinkProps = Omit<HTMLAnchorAttributes, "href"> &
  Appearance & {
    href: string;
  };
