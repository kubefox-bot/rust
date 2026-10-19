<script lang="ts">
  import Session from "./account/Session.svelte";
  import Password from "./account/Password.svelte";
  import Recovery from "./Recovery.svelte";

  let { oncommitted }: { oncommitted: () => void } = $props();
</script>

<div class="account-sections">
  <Session />
  <Password {oncommitted} />
  <Recovery {oncommitted} />
</div>

<style>
  .account-sections {
    display: grid;
    gap: 20px;
  }
  .account-sections :global(.card + .card) {
    margin-top: 0;
  }
</style>
