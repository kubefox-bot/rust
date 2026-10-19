<script lang="ts">
  import "./calendar.css";
  import { tick } from "svelte";
  import { Button } from "../button";
  import { Input } from "../input";
  import { formatServerDateTime } from "../../lib/time";
  import Calendar from "./Calendar.svelte";
  import { replaceDay, replaceTime } from "./calendar";
  import {
    POPOVER_WIDTH_PX,
    VIEWPORT_MARGIN_PX,
    POPOVER_GAP_PX,
    TIME_BASIS,
  } from "./conts";
  import type { DateTimePickerProps } from "./types";
  let {
    value,
    label,
    describedBy,
    timeBasis = "server",
    required = false,
    disabled = false,
    onchange,
  }: DateTimePickerProps = $props();
  const id = $props.id();
  let panel: HTMLDivElement | undefined = $state();
  let trigger: HTMLButtonElement | undefined = $state();
  let open = $state(false);
  let day = $state("");
  let time = $state("");
  let missing = $state(false);
  let left = $state(0);
  let top = $state(0);
  const candidate = $derived(
    day ? replaceTime({ value: replaceDay({ value, day }), time }) : null,
  );
  const place = () => {
    if (!panel || !trigger) return;
    const rect = trigger.getBoundingClientRect();
    const height = panel.getBoundingClientRect().height;
    const width = Math.min(
      POPOVER_WIDTH_PX,
      window.innerWidth - VIEWPORT_MARGIN_PX * 2,
    );
    left = Math.max(
      VIEWPORT_MARGIN_PX,
      Math.min(rect.left, window.innerWidth - width - VIEWPORT_MARGIN_PX),
    );
    const below =
      window.innerHeight - rect.bottom - POPOVER_GAP_PX - VIEWPORT_MARGIN_PX;
    const preferred =
      below >= height
        ? rect.bottom + POPOVER_GAP_PX
        : rect.top - height - POPOVER_GAP_PX;
    top = Math.max(
      VIEWPORT_MARGIN_PX,
      Math.min(preferred, window.innerHeight - height - VIEWPORT_MARGIN_PX),
    );
  };
  const show = () => {
    if (disabled) return;
    const now = new Date();
    const today = `${now.getFullYear().toString().padStart(4, "0")}-${(now.getMonth() + 1).toString().padStart(2, "0")}-${now.getDate().toString().padStart(2, "0")}`;
    day = value.slice(0, 10) || today;
    time = value.split("T")[1] || "00:00:00";
    panel?.showPopover();
  };
  const close = () => {
    panel?.hidePopover();
    trigger?.focus({ preventScroll: true });
  };
  const toggle = async () => {
    open = panel?.matches(":popover-open") ?? false;
    if (!open) return;
    await tick();
    place();
    panel
      ?.querySelector<HTMLButtonElement>('[data-calendar-day][tabindex="0"]')
      ?.focus({ preventScroll: true });
  };
  $effect(() => {
    if (disabled) panel?.hidePopover();
  });
  $effect(() => {
    if (!open) return;
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place);
    };
  });
</script>

<div class="date-time-picker">
  <button
    class="date-trigger"
    type="button"
    bind:this={trigger}
    {disabled}
    aria-label={label}
    aria-describedby={describedBy}
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-controls={id}
    data-datetime-value={value}
    onclick={() => (open ? close() : show())}
  >
    <span class:placeholder={!value}
      >{value ? formatServerDateTime(value) : "Выберите дату и время"}</span
    >
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      aria-hidden="true"
      ><rect
        x="3"
        y="5"
        width="18"
        height="16"
        rx="3"
        stroke="currentColor"
        stroke-width="1.6"
      /><path
        d="M7 3v4m10-4v4M3 11h18m-11 4h4"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
      /></svg
    >
  </button>
  <input
    class="sr-only"
    type="text"
    tabindex="-1"
    aria-hidden="true"
    {required}
    {disabled}
    {value}
    oninvalid={(event) => {
      event.preventDefault();
      missing = true;
      show();
    }}
  />
  <div
    bind:this={panel}
    {id}
    popover="auto"
    role="dialog"
    aria-label={label}
    class="date-popover"
    style:left={`${left}px`}
    style:top={`${top}px`}
    ontoggle={toggle}
  >
    {#if open && day}
      <div class="caption">
        {TIME_BASIS[timeBasis].caption}
      </div>
      <Calendar
        selected={day}
        onselect={(selected) => {
          day = selected;
        }}
      />
      <label class="field" for={`${id}-time`}
        >Время
        <Input
          id={`${id}-time`}
          aria-describedby={`${id}-time-hint`}
          type="text"
          inputmode="decimal"
          placeholder="ЧЧ:ММ:СС"
          value={time}
          aria-invalid={!candidate}
          oninput={(event) => (time = event.currentTarget.value)}
          onkeydown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              if (candidate) {
                onchange(candidate);
                missing = false;
                close();
              }
            }
          }}
        />
      </label>
      <p class="time-hint" id={`${id}-time-hint`}>
        {candidate
          ? TIME_BASIS[timeBasis].hint
          : "Введите время: ЧЧ:ММ или ЧЧ:ММ:СС. Доли секунды — через точку."}
      </p>
      {#if missing}<p class="validation" role="alert">
          Выберите дату и примените значение.
        </p>{/if}
      <div class="calendar-actions">
        <Button size="small" variant="quiet" onclick={close}>Отмена</Button>
        <Button
          size="small"
          variant="primary"
          disabled={!candidate}
          onclick={() => {
            if (candidate) {
              onchange(candidate);
              missing = false;
              close();
            }
          }}>Применить</Button
        >
      </div>
    {/if}
  </div>
</div>

<style>
  .date-time-picker {
    min-width: 0;
  }
  .date-trigger {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 40px;
    width: 100%;
    padding: 10px 12px;
    border: 1px solid var(--control-line);
    border-radius: 8px;
    background: var(--surface);
    color: var(--ink);
    text-align: left;
    font-size: 12px;
    cursor: pointer;
  }
  .date-trigger svg {
    flex-shrink: 0;
    color: var(--muted);
  }
  .date-trigger:hover:enabled {
    border-color: var(--accent-line);
  }
  .date-trigger:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 2px;
    border-color: var(--accent);
  }
  .date-trigger:disabled {
    background: #f6f8f8;
    color: var(--muted);
    cursor: not-allowed;
  }
  .placeholder {
    color: var(--muted);
  }
  .date-popover {
    position: fixed;
    margin: 0;
    width: 336px;
    max-width: calc(100vw - 24px);
    max-height: calc(100dvh - 24px);
    overflow: auto;
    padding: 16px;
    border: 1px solid var(--control-line);
    border-radius: 14px;
    background: var(--surface);
    color: var(--ink);
    box-shadow: 0 12px 40px #24323826;
  }
  .caption {
    font-size: 9px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: var(--muted);
    margin-bottom: 12px;
  }
  .time-hint {
    font-size: 10px;
    line-height: 1.5;
    color: var(--muted);
    margin: 7px 0 0;
  }
  .validation {
    color: var(--danger);
    font-size: 10px;
  }
  .calendar-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
</style>
