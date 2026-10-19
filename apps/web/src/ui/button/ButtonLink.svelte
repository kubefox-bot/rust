<script lang="ts">
  import "./button.css";
  import type { ButtonLinkProps } from "./types";

  let {
    href,
    variant = "secondary",
    size = "regular",
    class: className,
    children,
    ...attributes
  }: ButtonLinkProps = $props();
</script>

<a
  {...attributes}
  {href}
  class={["button", className]}
  data-variant={variant}
  data-size={size}
>
  {@render children?.()}
</a>
