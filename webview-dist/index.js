import { getCurrentHub } from "@sentry/core";
import { invoke } from "@tauri-apps/api/tauri";
export class TauriIntegration {
    constructor() {
        this.name = TauriIntegration.id;
    }
    setupOnce(addGlobalEventProcessor) {
        // Intercept events and sends them to Rust
        addGlobalEventProcessor(async (event) => {
            // The Sentry Rust type de-serialisation doesn't like these in their
            // current state
            delete event.sdk;
            delete event.breadcrumbs;
            // These will be overridden in the host
            delete event.environment;
            // This isn't in the Rust types
            delete event.sdkProcessingMetadata;
            await invoke("plugin:sentry|event", { event });
            // Stop events from being sent from the browser
            return null;
        });
        // Intercept global scope updates and send breadcrumbs to Rust
        const scope = getCurrentHub().getScope();
        if (scope) {
            scope.addScopeListener(async (updatedScope) => {
                // _breadcrumbs is private
                for (const breadcrumb of updatedScope._breadcrumbs) {
                    await invoke("plugin:sentry|breadcrumb", { breadcrumb });
                }
                // Clear so we don't send these again
                updatedScope.clearBreadcrumbs();
            });
        }
    }
}
TauriIntegration.id = "TauriIntegration";
