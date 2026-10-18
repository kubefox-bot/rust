<script lang="ts" generics="Value extends string | number">
  import type { SelectOption } from "./types";
  import { matchingOption, selectKey } from "./keyboard";
  import { menuPlacement } from "./placement";
  import { TYPEAHEAD_RESET_MS } from "./conts";
  import type { MenuPlacement } from "./types";

  const generatedId = $props.id();
  let {
    options,
    value = $bindable(),
    label,
    id = generatedId,
    menuLabel = "Доступные варианты",
    placeholder = "Выберите значение",
    disabled = false,
    onchange,
    "aria-label": ariaLabel,
    "aria-describedby": describedBy,
  }: {
    options: readonly SelectOption<Value>[];
    value?: Value;
    label?: string;
    id?: string;
    menuLabel?: string;
    placeholder?: string;
    disabled?: boolean;
    onchange?: (value: Value) => void;
    "aria-label"?: string;
    "aria-describedby"?: string;
  } = $props();

  let root: HTMLDivElement | undefined = $state(undefined);
  let trigger: HTMLButtonElement | undefined = $state(undefined);
  let open = $state(false);
  let activeIndex = $state(0);
  let placement = $state<MenuPlacement>({ side: "below", maxHeight: 0 });
  let query = "";
  let lastTypedAt = 0;
  const selectedIndex = $derived(
    options.findIndex((option) => option.value === value),
  );
  const selected = $derived(options[selectedIndex]);
  const unavailable = $derived(disabled || options.length === 0);
  const close = () => {
    open = false;
    query = "";
  };
  const placeMenu = () => {
    if (!trigger) return;
    const bounds = trigger.getBoundingClientRect();
    placement = menuPlacement({
      top: bounds.top,
      bottom: bounds.bottom,
      viewportHeight: window.innerHeight,
    });
  };
  const followViewport = () => {
    if (!open || !trigger) return;
    const bounds = trigger.getBoundingClientRect();
    if (bounds.bottom <= 0 || bounds.top >= window.innerHeight) close();
    else placeMenu();
  };
  const focusOption = (index: number) => {
    if (unavailable || !trigger) return;
    if (!open) placeMenu();
    activeIndex = index;
    open = true;
    trigger.focus({ preventScroll: true });
  };
  const choose = (index: number) => {
    const option = options[index];
    if (!option || unavailable) return;
    if (value !== option.value) {
      value = option.value;
      onchange?.(option.value);
    }
    close();
    trigger?.focus({ preventScroll: true });
  };
  const handleKey = (event: KeyboardEvent) => {
    if (unavailable || event.ctrlKey || event.metaKey) return;
    const action = selectKey({
      key: event.key,
      open,
      activeIndex,
      selectedIndex,
      count: options.length,
    });
    switch (action.kind) {
      case "focus":
        event.preventDefault();
        query = "";
        focusOption(action.index);
        return;
      case "commit":
        if (action.preventDefault) event.preventDefault();
        choose(action.index);
        return;
      case "close":
        event.preventDefault();
        close();
        return;
      case "none":
        break;
      default: {
        const unreachable: never = action;
        throw new Error(`Unexpected select action: ${unreachable}`);
      }
    }
    if (event.key.length !== 1 || event.altKey) return;
    event.preventDefault();
    const now = Date.now();
    const previousQuery = now - lastTypedAt < TYPEAHEAD_RESET_MS ? query : "";
    query = previousQuery === event.key ? event.key : previousQuery + event.key;
    lastTypedAt = now;
    const index = matchingOption({
      options,
      query,
      fromIndex: query.length > 1 ? activeIndex - 1 : activeIndex,
    });
    if (index !== null) focusOption(index);
  };
  const outside = (event: PointerEvent) => {
    if (event.target instanceof Node && !root?.contains(event.target)) close();
  };
  const leave = (event: FocusEvent) => {
    if (
      !(event.relatedTarget instanceof Node) ||
      !root?.contains(event.relatedTarget)
    )
      close();
  };
  $effect(() => {
    if (unavailable || !options[activeIndex]) close();
  });
  $effect(() => {
    if (open)
      root
        ?.querySelector(`#${CSS.escape(id)}-option-${activeIndex}`)
        ?.scrollIntoView({ block: "nearest" });
  });
</script>

<svelte:document onpointerdown={outside} />
<svelte:window onresize={followViewport} onscroll={followViewport} />

<div
  class="select"
  class:field={Boolean(label)}
  bind:this={root}
  onfocusout={leave}
>
  {#if label}<label for={id}>{label}</label>{/if}
  <div class="select-control">
    <button
      {id}
      bind:this={trigger}
      type="button"
      class="select-trigger"
      class:expanded={open}
      role="combobox"
      aria-label={ariaLabel}
      aria-describedby={describedBy}
      aria-haspopup="listbox"
      aria-expanded={open}
      aria-controls={`${id}-options`}
      aria-activedescendant={open ? `${id}-option-${activeIndex}` : undefined}
      disabled={unavailable}
      onkeydown={handleKey}
      onclick={() => (open ? close() : focusOption(Math.max(0, selectedIndex)))}
    >
      <span class="select-copy">
        <span class="select-value">{selected?.label ?? placeholder}</span>
        {#if selected?.description}<span class="select-description"
            >{selected?.description}</span
          >{/if}
      </span>
      <svg
        class="chevron"
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        aria-hidden="true"
      >
        <path
          d="m7 10 5 5 5-5"
          stroke="currentColor"
          stroke-width="1.7"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </button>
    {#if open}
      <div
        class="select-menu"
        class:above={placement.side === "above"}
        style:max-height={`${placement.maxHeight}px`}
      >
        <div class="menu-caption">{menuLabel}</div>
        <div
          id={`${id}-options`}
          role="listbox"
          aria-label={ariaLabel ?? label ?? menuLabel}
          class="select-options"
        >
          {#each options as option, index (option.value)}
            <button
              id={`${id}-option-${index}`}
              type="button"
              role="option"
              data-value={option.value}
              tabindex="-1"
              aria-selected={option.value === value}
              class="select-option"
              class:active={index === activeIndex}
              onpointerdown={(event) => event.preventDefault()}
              onpointermove={() => (activeIndex = index)}
              onclick={() => choose(index)}
            >
              <span class="select-copy">
                <span class="select-value">{option.label}</span>
                {#if option.description}<span class="select-description"
                    >{option.description}</span
                  >{/if}
              </span>
              {#if option.value === value}<svg
                  width="18"
                  height="18"
                  viewBox="0 0 24 24"
                  fill="none"
                  aria-hidden="true"
                  ><path
                    d="m5 12 4 4L19 6"
                    stroke="currentColor"
                    stroke-width="1.8"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  /></svg
                >{/if}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .select {
    min-width: 0;
  }
  .select-control {
    position: relative;
    min-width: 0;
  }
  .select-trigger,
  .select-option {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 18px;
    align-items: center;
    gap: 12px;
    width: 100%;
    text-align: left;
    color: var(--ink);
  }
  .select-trigger {
    min-height: 40px;
    padding: 9px 12px;
    border: 1px solid var(--control-line);
    border-radius: 8px;
    background: var(--surface);
  }
  .select-trigger:hover:not(:disabled) {
    border-color: var(--accent);
  }
  .select-trigger:focus-visible {
    border-color: var(--accent);
    outline: 2px solid var(--focus);
    outline-offset: 2px;
  }
  .select-trigger:disabled {
    background: #f6f8f8;
    color: var(--muted);
    opacity: 1;
  }
  .select-trigger.expanded {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .select-copy {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .select-value {
    font-size: 13px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  .select-description {
    color: var(--muted);
    font-size: 10px;
    font-weight: 400;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }
  .chevron {
    color: var(--muted);
    flex-shrink: 0;
  }
  .expanded .chevron {
    transform: rotate(180deg);
    color: var(--accent);
  }
  .select-menu {
    position: absolute;
    top: calc(100% + 8px);
    inset-inline: 0;
    z-index: 10;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 5px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface);
    box-shadow: 0 12px 32px #2432381f;
  }
  .select-menu.above {
    top: auto;
    bottom: calc(100% + 8px);
  }
  .menu-caption {
    padding: 8px 9px;
    color: var(--muted);
    font-size: 10px;
    font-weight: 500;
    flex-shrink: 0;
  }
  .select-options {
    overflow-y: auto;
    min-height: 0;
  }
  .select-option {
    padding: 10px 9px;
    border: 0;
    border-radius: 6px;
    background: transparent;
  }
  .select-option.active {
    background: var(--accent-soft);
  }
  .select-option[aria-selected="true"] {
    color: var(--accent);
  }
</style>
