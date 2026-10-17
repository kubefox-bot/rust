import * as validators from "./generated/validators.js";
import type { BrowserContracts } from "./generated/browser.types";
import type { ContractName } from "./types";
import type { ValidateFunction } from "ajv";
const registry: {
  [Name in ContractName]: ValidateFunction<BrowserContracts[Name]>;
} = validators;
export class ContractError extends Error {
  constructor() {
    super(
      "Ответ сервера не соответствует контракту. Обновите состояние страницы.",
    );
  }
}
export const decode = <Name extends ContractName>(
  name: Name,
  input: unknown,
): BrowserContracts[Name] => {
  const validate = registry[name];
  if (!validate(input)) throw new ContractError();
  return input;
};
