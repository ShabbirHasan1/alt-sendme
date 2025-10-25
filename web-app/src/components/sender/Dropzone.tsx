import { Upload, CheckCircle, Loader2 } from 'lucide-react'
import type { DropzoneProps } from '../../types/sender'

export function Dropzone({ 
  isDragActive, 
  selectedPath, 
  pathType, 
  isLoading
}: DropzoneProps) {
  const getDropzoneStyles = () => {
    const baseStyles = {
      border: '2px dashed',
      borderRadius: 'var(--radius-lg)',
      padding: '2rem',
      marginBottom: '1.5rem',
      textAlign: 'center' as const,
      cursor: 'pointer',
      transition: 'all 0.2s ease',
      backgroundColor: 'var(--app-main-view)',
      borderColor: 'rgba(255, 255, 255, 0.2)',
      color: 'var(--app-main-view-fg)',
    }

    if (isDragActive) {
      return {
        ...baseStyles,
        borderColor: 'var(--app-accent)',
        backgroundColor: 'rgba(45, 120, 220, 0.1)',
      }
    }

    if (selectedPath && !isLoading) {
      return {
        ...baseStyles,
      }
    }

    return baseStyles
  }

  const getStatusText = () => {
    if (isLoading) return 'Preparing for transport...'
    if (isDragActive) return 'Drop files or folders here'
    if (selectedPath) {
      if (pathType === 'directory') return 'Folder selected'
      if (pathType === 'file') return 'File selected'
      return 'Item selected'
    }
    return 'Drag & drop'
  }

  const getSubText = () => {
    if (isLoading) return 'Please wait while we process your files for sharing...'
    if (selectedPath) {
      return (
        <div 
          className="text-xs opacity-75 max-w-64 text-center mx-auto"
          style={{ 
            maxWidth: '16rem',
            overflow: 'hidden',
            whiteSpace: 'nowrap',
            textOverflow: 'ellipsis',
            direction: 'rtl',
            textAlign: 'left'
          }}
          title={selectedPath}
        >
          <span style={{ direction: 'ltr', unicodeBidi: 'bidi-override' }}>
            {selectedPath}
          </span>
        </div>
      )
    }
    return 'or browse to select files or folders'
  }

  return (
    <div style={getDropzoneStyles()}>
      <div className="space-y-4">
        <div className="flex justify-center">
          {isLoading ? (
            <Loader2 className="h-12 w-12 animate-spin" style={{ color: 'var(--app-accent-light)' }} />
          ) : selectedPath ? (
            <CheckCircle className="h-12 w-12" style={{ color: 'var(--app-primary)' }} />
          ) : (
            <Upload className="h-12 w-12" style={{ 
              color: isDragActive ? 'var(--app-accent-light)' : 'rgba(255, 255, 255, 0.6)' 
            }} />
          )}
        </div>
        
        <div>
          <p className="text-lg font-medium mb-2" style={{ color: 'var(--app-main-view-fg)' }}>
            {getStatusText()}
          </p>
          <div className="text-sm mb-4" style={{ color: 'rgba(255, 255, 255, 0.6)' }}>
            {getSubText()}
          </div>
        </div>
      </div>
    </div>
  )
}
