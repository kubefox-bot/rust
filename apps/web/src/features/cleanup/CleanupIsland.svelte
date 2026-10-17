<script lang="ts">
  import { onMount } from 'svelte';
  import { requestJson } from '../../api';
  let status = $state<'loading' | 'authenticated' | 'anonymous' | 'failed'>('loading');
  onMount(() => {
    const controller = new AbortController();
    requestJson({ path: '/auth/session', responseSchema: 'session', signal: controller.signal })
      .then(session => { status = session.authenticated ? 'authenticated' : 'anonymous'; })
      .catch(() => { if (!controller.signal.aborted) status = 'failed'; });
    return () => controller.abort();
  });
</script>
<section class="card" aria-label="Рабочая область очистки">
  <div class="empty" aria-live="polite">
    {#if status === 'loading'}Проверка сеанса…
    {:else if status === 'anonymous'}<p>Войдите, чтобы выбрать сервер и выполнить очистку.</p><a class="button primary" href="/login/">Войти в центр</a>
    {:else if status === 'failed'}Не удалось проверить сеанс. Обновите страницу.
    {:else}Рабочая область подключена к центру.{/if}
  </div>
</section>
