import { DELIVERY_TIMEOUT_MS } from "../conts";

export const initialize = async ({
  argv,
  cwd,
  login,
  password,
}: Readonly<{
  argv: readonly string[];
  cwd: string;
  login: string;
  password: string;
}>): Promise<void> => {
  let output = "";
  let step = 0;
  let recovery = "";
  const decoder = new TextDecoder();
  const child = Bun.spawn([...argv], {
    cwd,
    terminal: {
      cols: 160,
      rows: 30,
      data(terminal, data) {
        output += decoder.decode(data, { stream: true });
        const issued = /hmc-recovery-v1:[A-Za-z0-9_-]{43}/.exec(output);
        if (issued) recovery = issued[0];
        const prompts = [
          "Логин суперадмина: ",
          "Пароль: ",
          "Повторите пароль: ",
          "Введите сохранённый recovery key для подтверждения: ",
        ];
        const prompt = prompts[step];
        if (!prompt || !output.includes(prompt)) return;
        const values = [login, password, password, recovery];
        const value = values[step];
        if (!value) return;
        output = output.slice(output.indexOf(prompt) + prompt.length);
        step++;
        terminal.write(`${value}\n`);
      },
    },
  });
  const timer = setTimeout(() => child.kill(), DELIVERY_TIMEOUT_MS);
  try {
    const exitCode = await child.exited;
    if (exitCode !== 0 || step !== 4)
      throw new Error(
        `Disposable center initialization failed (exit ${exitCode}; completed prompts ${step}/4); secret terminal output was not retained`,
      );
  } finally {
    clearTimeout(timer);
    child.terminal?.close();
    output = "";
    recovery = "";
  }
};
