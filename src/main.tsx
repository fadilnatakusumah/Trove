/**
 * Entry point.
 *
 * The boot splash lives in `index.html` so the webview paints the app's
 * background instead of a white flash before this module even loads. Once
 * React has mounted, that splash is removed — keeping it around would leave a
 * spinner on screen forever if a route suspended.
 */
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { RouterProvider } from "react-router-dom";
import { Providers } from "@/app/providers";
import { router } from "@/app/router";
import "@/index.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Providers>
      <RouterProvider router={router} />
    </Providers>
  </StrictMode>,
);

document.getElementById("boot")?.remove();
