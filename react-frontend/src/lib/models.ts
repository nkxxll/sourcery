export type Codebase = {
  id: string
  name: string
  url: string
  programming_language: string
  created_at: string
}

export type CodebaseList = Codebase[]
export type CodebasesByLanguage = Record<string, Codebase[]>
