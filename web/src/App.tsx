import { useState } from "react";
import { HashRouter as Router, Switch, Route, Link } from "react-router-dom";
import Navbar from "react-bootstrap/Navbar";
import NavDropdown from "react-bootstrap/NavDropdown";
import Nav from "react-bootstrap/Nav";
import EmulatorPage from "./pages/emulator";
import AboutPage from "./pages/about";
// import JackCompilerPage from "./pages/jack-compiler";
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
        <Navbar.Brand href="/">Nand2Tetris Emulators</Navbar.Brand>
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
      <Switch>
        <Route path="/emulator/:demoId">
          <EmulatorPage />
        </Route>
        <Route path="/emulator">
          <EmulatorPage />
        </Route>
        <Route path="/">
          <AboutPage />
        </Route>
      </Switch>
    </Router>
  );
}

export default App;
