import "./Topbar.css";

type TopbarProps = {
  title: string;
  subtitle?: string;
};

export default function Topbar({ title, subtitle = "Flujo principal" }: TopbarProps) {
  return (
    <header className="topbar">
      <div>
        <h1>{title}</h1>
        <p>{subtitle}</p>
      </div>
    </header>
  );
}
