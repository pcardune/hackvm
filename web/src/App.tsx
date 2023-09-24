import { useState } from "react";
import { HashRouter as Router, Routes, Route, Link } from "react-router-dom";
import Navbar from "react-bootstrap/Navbar";
import NavDropdown from "react-bootstrap/NavDropdown";
import Nav from "react-bootstrap/Nav";
import EmulatorPage from "./pages/emulator";
import AboutPage from "./pages/about";
import demos from "./demos";

function App() {
  const [expanded, setExpanded] = useState(false);
  const onSelectDemo = () => {
    setExpanded(false);
  };
  const onToggle = (expanded: boolean) => {
    setExpanded(expanded);
  };
  return (
    <Router>
      <Navbar
        bg="light"
        expand="lg"
        className="mb-2"
        expanded={expanded}
        onToggle={onToggle}
      >
        <Navbar.Brand as={Link} to="/">
          Nand2Tetris VM Emulator
        </Navbar.Brand>
        <Navbar.Toggle />
        <Navbar.Collapse id="basic-navbar-nav">
          <Nav className="mr-auto" role="">
            <Nav.Link as={Link} to="/">
              About
            </Nav.Link>
            <NavDropdown
              title="Demos"
              id="basic-nav-dropdown"
              menuRole=""
              focusFirstItemOnShow={false}
            >
              {Object.keys(demos).map((demoId) => (
                <NavDropdown.Item
                  onSelect={onSelectDemo}
                  key={demoId}
                  as={Link}
                  to={`/emulator/${demoId}`}
                >
                  {demos[demoId].title}
                </NavDropdown.Item>
              ))}
            </NavDropdown>
          </Nav>
        </Navbar.Collapse>
      </Navbar>
      <Routes>
        <Route path="/emulator/:demoId" element={<EmulatorPage />} />
        <Route path="/emulator" element={<EmulatorPage />} />
        <Route path="/" element={<AboutPage />} />
      </Routes>
    </Router>
  );
}

export default App;
