import { Router, Route } from "@solidjs/router";

import Upload from "./Upload";
import Welcome from "./Welcome";

function App() {
    return <Router>
        <Route path="/" component={Welcome} />
        <Route path="/upload" component={Upload} />
    </Router>
}

export default App;