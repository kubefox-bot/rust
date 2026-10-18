<script lang="ts">
  import type { BrowserContracts } from "../../../api";
  import { Select } from "../../../ui/select";
  let {
    packages,
    value = $bindable(""),
    disabled = false,
  }: {
    packages: BrowserContracts["packages"];
    value?: string;
    disabled?: boolean;
  } = $props();
  const options = $derived(
    packages.map((candidate) => {
      const separator = candidate.build.indexOf("+");
      if (separator < 0)
        return { value: candidate.build, label: candidate.build };
      return {
        value: candidate.build,
        label: candidate.build.slice(0, separator),
        description: `Сборка ${candidate.build.slice(separator + 1)}`,
      };
    }),
  );
</script>

<Select
  label="Версия агента"
  menuLabel="Доступные версии"
  placeholder="Нет доступных версий"
  {options}
  bind:value
  {disabled}
/>
