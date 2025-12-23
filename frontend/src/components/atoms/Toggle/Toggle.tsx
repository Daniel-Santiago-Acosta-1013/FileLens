import "./Toggle.css";

type ToggleProps = {
  label: string;
  checked: boolean;
  disabled?: boolean;
  onChange: () => void;
};

export default function Toggle({ label, checked, disabled, onChange }: ToggleProps) {
  return (
    <label className="toggle">
      <input type="checkbox" checked={checked} disabled={disabled} onChange={() => onChange()} />
      <span>{label}</span>
    </label>
  );
}
