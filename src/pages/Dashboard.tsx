import { useState } from 'react'
import { cva } from 'class-variance-authority'
import { useAppStore } from '../store/useAppStore'

const buttonStyles = cva('btn', {
  variants: {
    tone: {
      primary: 'btn-primary',
      ghost: 'btn-ghost',
      subtle: 'btn-subtle',
    },
    size: {
      sm: 'btn-sm',
      md: 'btn-md',
    },
  },
  defaultVariants: {
    tone: 'primary',
    size: 'md',
  },
})

export function Dashboard() {
  const {
    transcripts,
    dictionary,
    addTranscript,
    updateDictionary,
    error,
    setError,
  } = useAppStore()
  const [snippet, setSnippet] = useState('')
  const [term, setTerm] = useState('')
  const [replacement, setReplacement] = useState('')

  const handleCopy = async (content: string) => {
    await navigator.clipboard.writeText(content)
  }

  const handleAddTranscript = () => {
    if (!snippet.trim()) return
    addTranscript(snippet.trim())
    setSnippet('')
  }

  const handleDictionaryUpdate = () => {
    if (!term.trim() || !replacement.trim()) return
    updateDictionary(term.trim(), replacement.trim())
    setTerm('')
    setReplacement('')
  }

  return (
    <div className="main-grid">
      {error && (
        <div className="error-banner" role="alert">
          {error}
          <button
            type="button"
            className="error-dismiss"
            onClick={() => setError(null)}
            aria-label="Dismiss"
          >
            ×
          </button>
        </div>
      )}

      <section className="panel panel-transcripts">
        <div className="panel-header">
          <div>
            <h2>Transcript dashboard</h2>
            <p className="panel-subtitle">
              Recent sessions and quick actions.
            </p>
          </div>
          <div className="panel-actions">
            <button
              className={buttonStyles({ tone: 'subtle', size: 'sm' })}
              type="button"
              onClick={handleAddTranscript}
            >
              Save snippet
            </button>
          </div>
        </div>

        <div className="input-card">
          <textarea
            value={snippet}
            onChange={(e) => setSnippet(e.target.value)}
            placeholder="Paste or type a transcript excerpt..."
            rows={4}
          />
        </div>

        <div className="list">
          {transcripts.map((item) => (
            <article key={item.id} className="list-item">
              <div>
                <p className="list-text">{item.content}</p>
                <p className="list-meta">{item.createdAt}</p>
              </div>
              <button
                className={buttonStyles({ tone: 'ghost', size: 'sm' })}
                type="button"
                onClick={() => handleCopy(item.content)}
              >
                Copy
              </button>
            </article>
          ))}
        </div>
      </section>

      <section className="panel panel-dictionary">
        <div className="panel-header">
          <div>
            <h2>Dictionary management</h2>
            <p className="panel-subtitle">
              Keep recurring fixes available offline.
            </p>
          </div>
        </div>

        <div className="dictionary-form">
          <div className="field">
            <label className="field-label" htmlFor="term">
              Term
            </label>
            <input
              id="term"
              value={term}
              onChange={(e) => setTerm(e.target.value)}
              placeholder="recieve"
              className="input"
            />
          </div>
          <div className="field">
            <label className="field-label" htmlFor="replacement">
              Replacement
            </label>
            <input
              id="replacement"
              value={replacement}
              onChange={(e) => setReplacement(e.target.value)}
              placeholder="receive"
              className="input"
            />
          </div>
          <button
            className={buttonStyles({ tone: 'primary', size: 'md' })}
            type="button"
            onClick={handleDictionaryUpdate}
          >
            Update dictionary
          </button>
        </div>

        <div className="list">
          {dictionary.map((entry) => (
            <article key={entry.id} className="list-item">
              <div>
                <p className="list-text">
                  <span className="badge">{entry.term}</span>
                  {entry.replacement}
                </p>
                <p className="list-meta">{entry.createdAt}</p>
              </div>
              <button
                className={buttonStyles({ tone: 'ghost', size: 'sm' })}
                type="button"
                onClick={() => handleCopy(entry.replacement)}
              >
                Copy
              </button>
            </article>
          ))}
        </div>
      </section>
    </div>
  )
}
