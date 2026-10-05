import { BrowserRouter, NavLink, Route, Routes } from "react-router";
import ReviewPage from "./review/ReviewPage";
import TreePage from "./tree/TreePage";

export default function App() {
  return (
    <BrowserRouter>
      <div className="mx-auto flex min-h-screen w-full max-w-xl flex-col gap-6 px-4 py-6">
        <nav className="flex gap-4 text-sm">
          <NavLink to="/" end className={navClass}>
            復習
          </NavLink>
          <NavLink to="/tree" className={navClass}>
            ツリー
          </NavLink>
        </nav>
        <Routes>
          <Route path="/" element={<ReviewPage />} />
          <Route path="/tree" element={<TreePage />} />
        </Routes>
      </div>
    </BrowserRouter>
  );
}

function navClass({ isActive }: { isActive: boolean }) {
  return isActive ? "font-semibold underline" : "text-neutral-500";
}
