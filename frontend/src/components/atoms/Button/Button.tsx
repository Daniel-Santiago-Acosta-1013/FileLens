import type { ButtonHTMLAttributes } from "react";
import "./Button.css";

type ButtonVariant = "primary" | "secondary" | "danger";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: ButtonVariant;
};

export default function Button({ variant = "secondary", className = "", ...props }: ButtonProps) {
  const classes = `${variant} ${className}`.trim();
  return <button className={classes} {...props} />;
}
