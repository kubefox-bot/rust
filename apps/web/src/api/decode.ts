import type { ValidateFunction } from "ajv";
export class ContractError extends Error {
  constructor() {
    super(
      "Ответ сервера не соответствует контракту. Обновите состояние страницы.",
    );
  }
}
export const decode = <Value>(
  validate: ValidateFunction<Value>,
  input: unknown,
): Value => {
  if (!validate(input)) throw new ContractError();
  return input;
};
