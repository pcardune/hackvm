import React from "react";
import { render, screen } from "@testing-library/react";
import App from "./App";

test("renders a header", () => {
  render(<App />);
  const linkElement = screen.getByText(/Hack/i);
  expect(linkElement).toBeInTheDocument();
});
