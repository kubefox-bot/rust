export type DateTimePickerProps = Readonly<{
  value: string;
  label: string;
  timeBasis?: "server" | "browser";
  describedBy?: string;
  required?: boolean;
  disabled?: boolean;
  onchange: (value: string) => void;
}>;
