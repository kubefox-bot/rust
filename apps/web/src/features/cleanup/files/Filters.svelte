<script lang="ts">
  import { DateTimePicker } from "../../../ui/date-time";
  import { Select } from "../../../ui/select";
  import { Button } from "../../../ui/button";
  import { RELATION_OPTIONS } from "./conts";
  import type { FileFilters } from "./types";
  let {
    value,
    onchange,
    onreset,
  }: {
    value: FileFilters;
    onchange: (value: FileFilters) => void;
    onreset: () => void;
  } = $props();
</script>

<div class="filters">
  <div class="date-filter">
    <span>Создан с</span>
    <DateTimePicker
      label="Создан с"
      value={value.from}
      timeBasis="browser"
      onchange={(from) => onchange({ ...value, from })}
    />
  </div>
  <div class="date-filter">
    <span>Создан по</span>
    <DateTimePicker
      label="Создан по"
      value={value.to}
      timeBasis="browser"
      onchange={(to) => onchange({ ...value, to })}
    />
  </div>
  <Select
    label="Связь с базой"
    options={RELATION_OPTIONS}
    value={value.relation}
    onchange={(relation) => onchange({ ...value, relation })}
  />
  <Button variant="quiet" onclick={onreset}>Сбросить фильтры</Button>
</div>

<style>
  .filters {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr)) minmax(130px, 0.65fr) auto;
    gap: 12px;
    align-items: end;
    margin-top: 16px;
  }
  .date-filter {
    display: flex;
    flex-direction: column;
    gap: 7px;
    min-width: 0;
    font-size: 11px;
    font-weight: 550;
  }
  @media (max-width: 800px) {
    .filters {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (max-width: 480px) {
    .filters {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
