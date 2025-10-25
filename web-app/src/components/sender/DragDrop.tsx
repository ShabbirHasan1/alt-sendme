import { useEffect } from 'react'
import { Dropzone } from './Dropzone'
import { BrowseButtons } from './BrowseButtons'
import { AppAlertDialog } from '../AppAlertDialog'
import { useDragDrop } from '../../hooks/useDragDrop'

interface DragDropProps {
  onFileSelect: (path: string) => void
  selectedPath?: string | null
  isLoading?: boolean
}

export function DragDrop({ onFileSelect, selectedPath, isLoading }: DragDropProps) {
  console.log('📁 DragDrop component rendered');
  console.log('📁 Selected path:', selectedPath);
  console.log('📁 Is loading:', isLoading);
  
  const {
    isDragActive,
    pathType,
    alertDialog,
    browseFile,
    browseFolder,
    closeAlert,
    checkPathType
  } = useDragDrop(onFileSelect)

  // Check path type when selectedPath changes
  useEffect(() => {
    if (selectedPath) {
      console.log('🔍 Checking path type for:', selectedPath);
      checkPathType(selectedPath)
    }
  }, [selectedPath, checkPathType])

  return (
    <div>
      <Dropzone
        isDragActive={isDragActive}
        selectedPath={selectedPath || null}
        pathType={pathType}
        isLoading={isLoading || false}
      />

      {!selectedPath && (
        <BrowseButtons
          isLoading={isLoading || false}
          onBrowseFile={browseFile}
          onBrowseFolder={browseFolder}
        />
      )}

      <AppAlertDialog
        isOpen={alertDialog.isOpen}
        title={alertDialog.title}
        description={alertDialog.description}
        type={alertDialog.type}
        onClose={closeAlert}
      />
    </div>
  )
}
