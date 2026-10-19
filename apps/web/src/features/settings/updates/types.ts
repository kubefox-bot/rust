import type { BrowserContracts } from "../../../api";
export type Policy = BrowserContracts["update_policy"];
export type PolicyState =
  | { readonly kind: "loading" }
  | { readonly kind: "failed"; readonly message: string }
  | {
      readonly kind: "ready";
      readonly policy: Policy;
      readonly saving: boolean;
      readonly saved: boolean;
      readonly error: string;
    };
