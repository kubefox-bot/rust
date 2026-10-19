<script lang="ts">
  import type { CheckboxProps } from "./types";

  let {
    label,
    description,
    checked = $bindable(false),
    tone = "default",
    class: className,
    ...attributes
  }: CheckboxProps = $props();
</script>

<label class={["checkbox-field", className]} data-tone={tone}>
  <span class="checkbox-control">
    <input {...attributes} type="checkbox" bind:checked />
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="m3.5 8 3 3 6-6"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  </span>
  <span class="checkbox-copy">
    <span>{label}</span>
    {#if description}<small>{description}</small>{/if}
  </span>
</label>

<style>
  .checkbox-field {
    --checkbox-color: var(--accent);
    display: flex;
    align-items: flex-start;
    gap: 10px;
    min-height: 24px;
    color: var(--ink);
    font-size: 12px;
    line-height: 1.6;
    cursor: pointer;
  }
  .checkbox-field[data-tone="danger"] {
    --checkbox-color: var(--danger);
  }
  .checkbox-control {
    position: relative;
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    margin-top: 1px;
  }
  input {
    appearance: none;
    width: 18px;
    height: 18px;
    margin: 0;
    border: 1px solid var(--control-line);
    border-radius: 5px;
    background: var(--surface);
    cursor: inherit;
  }
  input:checked {
    background: var(--checkbox-color);
    border-color: var(--checkbox-color);
  }
  .checkbox-field:hover input:enabled {
    border-color: var(--checkbox-color);
  }
  input:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  svg {
    position: absolute;
    inset: 2px;
    width: 14px;
    height: 14px;
    color: #fff;
    opacity: 0;
    pointer-events: none;
  }
  input:checked + svg {
    opacity: 1;
  }
  .checkbox-field:has(input:disabled) {
    color: var(--muted);
    cursor: not-allowed;
  }
  .checkbox-control:has(input:disabled) {
    opacity: 0.5;
  }
  input:disabled:not(:checked) {
    background: var(--line);
  }
  .checkbox-copy {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  small {
    display: block;
    margin-top: 3px;
    color: var(--muted);
    font-size: 10px;
    font-weight: 400;
    line-height: 1.5;
  }
  @media (forced-colors: active) {
    input:checked {
      background: Highlight;
      border-color: Highlight;
    }
    svg {
      color: HighlightText;
    }
  }
</style>
