import "./SegmentedControl.css";

type SegmentedOption<T extends string> = {
  id: T;
  label: string;
};

type SegmentedControlProps<T extends string> = {
  value: T;
  options: SegmentedOption<T>[];
  onChange: (value: T) => void;
  className?: string;
};

export default function SegmentedControl<T extends string>({
  value,
  options,
  onChange,
  className = ""
}: SegmentedControlProps<T>) {
  return (
    <div className={`segmented ${className}`.trim()}>
      {options.map((option) => (
        <button
          key={option.id}
          className={value === option.id ? "active" : ""}
          onClick={() => onChange(option.id)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
