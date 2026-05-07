import { flexRender, getCoreRowModel, useReactTable } from '@tanstack/react-table'
import type { ColumnDef } from '@tanstack/react-table'
import { useMemo } from 'react'

import { Button } from '#/components/ui/button'
import type { Codebase } from '#/lib/models'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '#/components/ui/table'

type LanguageRows = Record<string, Codebase[]>

export function LanguageTable({ rows }: { rows: LanguageRows }) {
  const languages = useMemo(() => Object.keys(rows), [rows])
  const data = useMemo(() => [rows], [rows])

  const columns = useMemo<ColumnDef<LanguageRows>[]>(
    () =>
      languages.map((language) => ({
        accessorKey: language,
        header: () => (
          <div className="py-2">
            <div className="m-4 border-b-2 border-gray-400" />
            <h2 className="mx-8 text-xl font-bold">{language}</h2>
            <div className="m-4 border-t-2 border-gray-400" />
          </div>
        ),
        cell: ({ getValue }) => {
          const codebases = getValue<Codebase[]>()

          return (
            <ul className="m-0 flex list-none flex-col gap-2 p-4">
              {codebases.map((codebase) => (
                <li key={codebase.id}>
                  <Button className="align-middle" size="lg">
                    {codebase.name}
                  </Button>
                </li>
              ))}
            </ul>
          )
        },
      })),
    [languages],
  )

  const table = useReactTable({
    data,
    columns,
    getCoreRowModel: getCoreRowModel(),
  })

  return (
    <Table className="w-full border border-red-600 table-fixed">
      <TableHeader>
        {table.getHeaderGroups().map((headerGroup) => (
          <TableRow key={headerGroup.id}>
            {headerGroup.headers.map((header) => (
              <TableHead
                key={header.id}
                className="border border-red-600 p-0 align-top"
              >
                {header.isPlaceholder
                  ? null
                  : flexRender(
                      header.column.columnDef.header,
                      header.getContext(),
                    )}
              </TableHead>
            ))}
          </TableRow>
        ))}
      </TableHeader>
      <TableBody>
        {table.getRowModel().rows.map((row) => (
          <TableRow key={row.id}>
            {row.getVisibleCells().map((cell) => (
              <TableCell
                key={cell.id}
                className="border border-red-600 p-0 align-top"
              >
                {flexRender(cell.column.columnDef.cell, cell.getContext())}
              </TableCell>
            ))}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
