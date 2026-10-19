<script lang="ts">
  import "./button.css";
  import type { ButtonProps } from "./types";

  let {
    variant = "secondary",
    size = "regular",
    loading = false,
    disabled = false,
    type = "button",
    element = $bindable(),
    class: className,
    children,
    ...attributes
  }: ButtonProps = $props();
</script>

<button
  {...attributes}
  {type}
  disabled={disabled || loading}
  aria-busy={loading || attributes["aria-busy"]}
  class={["button", className]}
  data-variant={variant}
  data-size={size}
  bind:this={element}
>
  {#if loading}<span class="button-spinner" aria-hidden="true"></span>{/if}
  {@render children?.()}
</button>
