import * as React from "react"
import { Search } from "lucide-react"

import { cn } from "@/lib/utils"

function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <input
      type={type}
      data-slot="input"
      className={cn("nv-input", className)}
      {...props}
    />
  )
}

function SearchInput({ className, inputClassName, ...props }: React.ComponentProps<"input"> & { inputClassName?: string }) {
  return (
    <div className={cn("nv-search", className)}>
      <Search strokeWidth={2} aria-hidden />
      <Input type="search" className={inputClassName} {...props} />
    </div>
  )
}

export { Input, SearchInput }
