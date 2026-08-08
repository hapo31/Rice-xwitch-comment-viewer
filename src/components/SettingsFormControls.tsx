import { Save } from "lucide-react";
import type { ReactNode } from "react";
import { focusIndicatorClass } from "../presentation/focus";

export function SettingsSection({
  id,
  title,
  children,
}: {
  id: string;
  title: string;
  children: ReactNode;
}) {
  const headingId = `${id}-heading`;

  return (
    <section aria-labelledby={headingId} className="border-y border-zinc-800">
      <h2 id={headingId} className="border-b border-zinc-800 px-3 py-2 text-xs font-semibold text-zinc-300">
        {title}
      </h2>
      {children}
    </section>
  );
}

export function FloatingSaveButton({
  visible,
  disabled,
  onClick,
}: {
  visible: boolean;
  disabled: boolean;
  onClick: () => void;
}) {
  if (!visible) {
    return null;
  }

  return (
    <div className="absolute bottom-4 right-4 z-10 transition-all duration-200 ease-out translate-x-0 translate-y-0 opacity-100">
      <button
        type="button"
        aria-label="設定を保存"
        title="設定を保存"
        disabled={disabled}
        onClick={onClick}
        className="flex h-10 items-center gap-2 border border-sky-500 bg-sky-500 px-4 text-sm font-medium text-zinc-950 shadow-lg shadow-zinc-950/40 hover:border-sky-300 hover:bg-sky-400 disabled:cursor-not-allowed disabled:border-zinc-700 disabled:bg-zinc-800 disabled:text-zinc-400"
      >
        <Save className="h-4 w-4" />
        保存
      </button>
    </div>
  );
}

export function ToggleRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className="grid grid-cols-[180px_minmax(0,1fr)] items-center border-b border-zinc-800 py-3 last:border-b-0">
      <span className="text-sm text-zinc-400">{label}</span>
      <span className="flex items-center gap-2 text-sm text-zinc-200">
        <input
          type="checkbox"
          checked={checked}
          onChange={(event) => onChange(event.target.checked)}
          className="h-4 w-4 accent-sky-400"
        />
        {checked ? "ON" : "OFF"}
      </span>
    </label>
  );
}

export function NumberRuleRow({
  id,
  label,
  value,
  valid,
  error,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  valid: boolean;
  error: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3 last:border-b-0">
      <label className="pt-2 text-sm text-zinc-400" htmlFor={id}>
        {label}
      </label>
      <div>
        <input
          id={id}
          inputMode="numeric"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          className={`h-9 w-40 border border-zinc-700 bg-zinc-900 px-3 text-sm text-zinc-100 ${focusIndicatorClass}`}
        />
        {!valid && <p className="mt-1 text-xs text-rose-400">{error}</p>}
      </div>
    </div>
  );
}

export function RuleTextArea({
  id,
  label,
  value,
  onChange,
  itemCount,
  overflowCount,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  itemCount: number;
  overflowCount: number;
}) {
  const isValid = overflowCount === 0;

  return (
    <div className="grid grid-cols-[180px_minmax(0,1fr)] items-start border-b border-zinc-800 py-3 last:border-b-0">
      <label className="pt-2 text-sm text-zinc-400" htmlFor={id}>
        {label}
      </label>
      <div>
        <textarea
          id={id}
          value={value}
          rows={5}
          aria-invalid={!isValid}
          aria-describedby={`${id}-status`}
          onChange={(event) => onChange(event.target.value)}
          className={`resize-y border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-100 ${focusIndicatorClass}`}
        />
        <p id={`${id}-status`} className={`mt-1 text-xs ${isValid ? "text-zinc-400" : "text-rose-400"}`}>
          {isValid
            ? `${itemCount}/200 件`
            : `${itemCount}/200 件（${overflowCount} 件超過）。上限を超えているため保存できません。`}
        </p>
      </div>
    </div>
  );
}

export function RangeRow({
  id,
  label,
  value,
  min,
  max,
  onChange,
}: {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  return (
    <div className="grid grid-cols-[180px_minmax(0,1fr)_64px] items-center border-b border-zinc-800 py-3 last:border-b-0">
      <label className="text-sm text-zinc-400" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        type="range"
        min={min}
        max={max}
        value={value}
        aria-valuetext={value === -1 ? "既定" : String(value)}
        onChange={(event) => onChange(Number(event.target.value))}
        className="w-full accent-sky-400"
      />
      <span className="text-right font-mono text-xs text-zinc-300">{value === -1 ? "既定" : value}</span>
    </div>
  );
}
