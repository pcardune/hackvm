import type { ButtonProps } from "react-bootstrap";
import Button from "react-bootstrap/Button";

type IconButtonProps = ButtonProps & { icon: string; label: string };
const IconButton = ({ icon, label, ...props }: IconButtonProps) => {
  return (
    <Button {...props}>
      <i className={"bi-" + icon} role="img" aria-label={label} />
      {label}
    </Button>
  );
};

export default IconButton;
