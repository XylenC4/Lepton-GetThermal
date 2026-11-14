import { ThemeMode } from "../hooks/useTheme";

const OPTIONS: { value: ThemeMode; label: string }[] = [
  { value: "system", label: "Auto" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

interface ThemeSwitcherProps {
  mode: ThemeMode;
  onChange: (mode: ThemeMode) => void;
}

export function ThemeSwitcher({ mode, onChange }: ThemeSwitcherProps) {
  return (
    <div className="control-section">
      <h3>Theme</h3>
      <div className="segmented" role="radiogroup" aria-label="Theme">
        {OPTIONS.map((o) => (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={mode === o.value}
            className={`segmented-option ${mode === o.value ? "active" : ""}`}
            onClick={() => onChange(o.value)}
            title={o.value === "system" ? "Follow the Windows app theme" : undefined}
          >
            {o.label}
          </button>
        ))}
      </div>
    </div>
  );
}
