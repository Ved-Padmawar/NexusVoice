import * as React from "react"
import { motion } from "framer-motion"
import { Tabs as TabsPrimitive } from "radix-ui"

import { cn } from "@/lib/utils"

const TabsContext = React.createContext<{ value: string; layoutId: string } | null>(null)

/** Controlled only — the active trigger draws the sliding indicator. */
function Tabs({ value, className, ...props }: React.ComponentProps<typeof TabsPrimitive.Root> & { value: string }) {
  const layoutId = React.useId()
  return (
    <TabsContext.Provider value={{ value, layoutId }}>
      <TabsPrimitive.Root data-slot="tabs" value={value} className={className} {...props} />
    </TabsContext.Provider>
  )
}

function TabsList({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.List>) {
  return <TabsPrimitive.List data-slot="tabs-list" className={cn("nv-tabs__list", className)} {...props} />
}

function TabsTrigger({ className, value, children, ...props }: React.ComponentProps<typeof TabsPrimitive.Trigger>) {
  const ctx = React.useContext(TabsContext)
  return (
    <TabsPrimitive.Trigger data-slot="tabs-trigger" value={value} className={cn("nv-tabs__trigger", className)} {...props}>
      {children}
      {ctx?.value === value && (
        <motion.span
          layoutId={ctx.layoutId}
          className="nv-tabs__ind"
          transition={{ type: "spring", stiffness: 480, damping: 40 }}
        />
      )}
    </TabsPrimitive.Trigger>
  )
}

function TabsContent({ className, ...props }: React.ComponentProps<typeof TabsPrimitive.Content>) {
  return <TabsPrimitive.Content data-slot="tabs-content" className={cn("nv-tab-panel", className)} {...props} />
}

export { Tabs, TabsList, TabsTrigger, TabsContent }
