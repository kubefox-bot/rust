<script lang="ts">
  import { tick, untrack } from "svelte";
  import { Button } from "../button";
  import { Input } from "../input";
  import { monthDays, calendarKey, shiftMonth } from "./calendar";
  import {
    WEEKDAYS,
    DAYS_PER_WEEK,
    CALENDAR_LOCALE,
    MONTH_OPTIONS,
    DAY_OPTIONS,
    MIN_YEAR,
    MAX_YEAR,
  } from "./conts";
  let {
    selected,
    onselect,
  }: { selected: string; onselect: (day: string) => void } = $props();
  let displayed = $state(untrack(() => selected));
  let focused = $state(untrack(() => selected));
  let grid: HTMLTableElement | undefined = $state();
  const monthFormat = new Intl.DateTimeFormat(CALENDAR_LOCALE, MONTH_OPTIONS);
  const dayFormat = new Intl.DateTimeFormat(CALENDAR_LOCALE, DAY_OPTIONS);
  const days = $derived(monthDays(displayed));
  const weeks = $derived(
    Array.from({ length: days.length / DAYS_PER_WEEK }, (_, index) =>
      days.slice(index * DAYS_PER_WEEK, (index + 1) * DAYS_PER_WEEK),
    ),
  );
  $effect(() => {
    displayed = selected;
    focused = selected;
  });
  const changeMonth = (months: number) => {
    displayed = shiftMonth({ day: displayed, months });
    focused = displayed;
  };
  const keydown = async (event: KeyboardEvent) => {
    if (
      !(event.target instanceof HTMLButtonElement) ||
      !event.target.dataset.calendarDay ||
      event.altKey ||
      event.ctrlKey ||
      event.metaKey
    )
      return;
    const day = event.target.dataset.calendarDay;
    const next = calendarKey({ day, key: event.key, shift: event.shiftKey });
    if (!next) return;
    event.preventDefault();
    focused = next;
    if (next.slice(0, 7) !== displayed.slice(0, 7)) displayed = next;
    await tick();
    grid
      ?.querySelector<HTMLButtonElement>(`[data-calendar-day="${next}"]`)
      ?.focus();
  };
</script>

<div class="date-calendar">
  <div class="calendar-heading" data-calendar-month={displayed.slice(0, 7)}>
    <Button
      size="small"
      variant="quiet"
      aria-label="Предыдущий месяц"
      onclick={() => changeMonth(-1)}>‹</Button
    >
    <span class="month" aria-live="polite"
      >{monthFormat.format(new Date(`${displayed}T00:00:00Z`))}</span
    >
    <Input
      aria-label="Год календаря"
      type="number"
      min={MIN_YEAR}
      max={MAX_YEAR}
      value={Number(displayed.slice(0, 4))}
      onkeydown={(event) => {
        if (event.key === "Enter") event.preventDefault();
      }}
      onchange={(event) => {
        const year = Number(event.currentTarget.value);
        if (!Number.isInteger(year) || year < MIN_YEAR || year > MAX_YEAR)
          return;
        displayed = shiftMonth({
          day: displayed,
          months: (year - Number(displayed.slice(0, 4))) * 12,
        });
        focused = displayed;
      }}
    />
    <Button
      size="small"
      variant="quiet"
      aria-label="Следующий месяц"
      onclick={() => changeMonth(1)}>›</Button
    >
  </div>
  <table
    role="grid"
    aria-label="Выбор даты"
    bind:this={grid}
    onkeydown={keydown}
  >
    <thead
      ><tr
        >{#each WEEKDAYS as name}<th scope="col">{name}</th>{/each}</tr
      ></thead
    >
    <tbody
      >{#each weeks as week}<tr
          >{#each week as day}<td
              role="gridcell"
              aria-selected={day === selected}
            >
              {#if day}<button
                  type="button"
                  data-calendar-day={day}
                  class:outside={day.slice(0, 7) !== displayed.slice(0, 7)}
                  class:selected={day === selected}
                  tabindex={day === focused ? 0 : -1}
                  aria-label={dayFormat.format(new Date(`${day}T00:00:00Z`))}
                  onclick={() => onselect(day)}>{Number(day.slice(-2))}</button
                >{/if}
            </td>{/each}</tr
        >{/each}</tbody
    >
  </table>
</div>
