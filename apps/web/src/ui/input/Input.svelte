<script lang="ts" generics="Value extends string | number | undefined">
  import type { InputProps } from "./types";

  let {
    value = $bindable(),
    element = $bindable(),
    type = "text",
    class: className,
    ...attributes
  }: InputProps<Value> = $props();
</script>

<input
  {...attributes}
  {type}
  class={["input", className]}
  bind:value
  bind:this={element}
/>

<style>
  .input {
    width: 100%;
    min-width: 0;
    min-height: 40px;
    padding: 10px 12px;
    border: 1px solid var(--control-line);
    border-radius: 8px;
    background: var(--surface);
    color: var(--ink);
    font-size: 12px;
    line-height: 1.5;
  }
  .input:hover:enabled:not([readonly]) {
    border-color: var(--accent-line);
  }
  .input:focus-visible {
    border-color: var(--accent);
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .input:disabled {
    background: #f6f8f8;
    color: var(--muted);
    cursor: not-allowed;
  }
</style>
