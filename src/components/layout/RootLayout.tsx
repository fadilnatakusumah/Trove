import { Outlet } from "react-router-dom";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { AppSidebar } from "@/components/layout/AppSidebar";
import { CommandPalette } from "@/components/layout/CommandPalette";

export function RootLayout() {
  return (
    <SidebarProvider>
      <AppSidebar />
      <SidebarInset className="min-h-svh">
        <Outlet />
      </SidebarInset>
      {/* Inside the router context here — useNavigate works. */}
      <CommandPalette />
    </SidebarProvider>
  );
}
