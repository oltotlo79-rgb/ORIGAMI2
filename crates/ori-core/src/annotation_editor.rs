use ori_domain::{
    AnnotationAnchorV1, AnnotationDocumentV1, AnnotationId, AnnotationRecordV1, CreasePattern,
    LayerContentKindV1, LayerId, Point2, ProjectLayerDocumentV1, VertexId,
    validate_annotation_document_v1,
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum AnnotationEditErrorV1 {
    #[error("annotation revision is stale")]
    Stale,
    #[error("annotation document is invalid")]
    Invalid,
    #[error("annotation already exists")]
    Duplicate,
    #[error("annotation was not found")]
    Missing,
    #[error("annotation layer is missing, locked, or has the wrong kind")]
    LayerUnavailable,
    #[error("annotation anchor vertex is missing")]
    AnchorVertexMissing,
    #[error("vertex is referenced by an annotation")]
    AnchorVertexInUse,
    #[error("layer is referenced by an annotation")]
    LayerInUse,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnnotationEditorV1 {
    document: AnnotationDocumentV1,
    revision: u64,
    undo: Vec<AnnotationDocumentV1>,
    redo: Vec<AnnotationDocumentV1>,
}

impl AnnotationEditorV1 {
    #[must_use]
    pub fn new(document: AnnotationDocumentV1) -> Self {
        Self {
            document,
            revision: 0,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    #[must_use]
    pub const fn document(&self) -> &AnnotationDocumentV1 {
        &self.document
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn add(
        &mut self,
        expected: u64,
        record: AnnotationRecordV1,
        pattern: &CreasePattern,
        layers: &ProjectLayerDocumentV1,
    ) -> Result<(), AnnotationEditErrorV1> {
        self.mutate(expected, pattern, layers, |document| {
            if document.annotations.iter().any(|item| item.id == record.id) {
                return Err(AnnotationEditErrorV1::Duplicate);
            }
            document.annotations.push(record);
            Ok(())
        })
    }

    pub fn update(
        &mut self,
        expected: u64,
        record: AnnotationRecordV1,
        pattern: &CreasePattern,
        layers: &ProjectLayerDocumentV1,
    ) -> Result<(), AnnotationEditErrorV1> {
        self.mutate(expected, pattern, layers, |document| {
            let current = document
                .annotations
                .iter_mut()
                .find(|item| item.id == record.id)
                .ok_or(AnnotationEditErrorV1::Missing)?;
            *current = record;
            Ok(())
        })
    }

    pub fn remove(&mut self, expected: u64, id: AnnotationId) -> Result<(), AnnotationEditErrorV1> {
        if self.revision != expected {
            return Err(AnnotationEditErrorV1::Stale);
        }
        let index = self
            .document
            .annotations
            .iter()
            .position(|item| item.id == id)
            .ok_or(AnnotationEditErrorV1::Missing)?;
        let before = self.document.clone();
        self.document.annotations.remove(index);
        self.commit(before);
        Ok(())
    }

    pub fn undo(&mut self, expected: u64) -> Result<(), AnnotationEditErrorV1> {
        if self.revision != expected {
            return Err(AnnotationEditErrorV1::Stale);
        }
        let previous = self.undo.pop().ok_or(AnnotationEditErrorV1::Missing)?;
        self.redo
            .push(std::mem::replace(&mut self.document, previous));
        self.revision += 1;
        Ok(())
    }

    pub fn redo(&mut self, expected: u64) -> Result<(), AnnotationEditErrorV1> {
        if self.revision != expected {
            return Err(AnnotationEditErrorV1::Stale);
        }
        let next = self.redo.pop().ok_or(AnnotationEditErrorV1::Missing)?;
        self.undo.push(std::mem::replace(&mut self.document, next));
        self.revision += 1;
        Ok(())
    }

    pub fn ensure_vertex_removable(&self, vertex: VertexId) -> Result<(), AnnotationEditErrorV1> {
        if self.document.annotations.iter().any(|annotation| {
            matches!(annotation.anchor, AnnotationAnchorV1::Vertex { vertex: id, .. } if id == vertex)
        }) { Err(AnnotationEditErrorV1::AnchorVertexInUse) } else { Ok(()) }
    }

    pub fn ensure_layer_removable(&self, layer: LayerId) -> Result<(), AnnotationEditErrorV1> {
        if self
            .document
            .annotations
            .iter()
            .any(|annotation| annotation.layer == layer)
        {
            Err(AnnotationEditErrorV1::LayerInUse)
        } else {
            Ok(())
        }
    }

    pub fn resolved_anchor(
        &self,
        annotation: &AnnotationRecordV1,
        pattern: &CreasePattern,
    ) -> Result<Point2, AnnotationEditErrorV1> {
        match annotation.anchor {
            AnnotationAnchorV1::Absolute { position } => Ok(position),
            AnnotationAnchorV1::Vertex { vertex, offset } => {
                let position = pattern
                    .vertices
                    .iter()
                    .find(|item| item.id == vertex)
                    .map(|item| item.position)
                    .ok_or(AnnotationEditErrorV1::AnchorVertexMissing)?;
                Ok(Point2::new(position.x + offset.x, position.y + offset.y))
            }
        }
    }

    fn mutate(
        &mut self,
        expected: u64,
        pattern: &CreasePattern,
        layers: &ProjectLayerDocumentV1,
        apply: impl FnOnce(&mut AnnotationDocumentV1) -> Result<(), AnnotationEditErrorV1>,
    ) -> Result<(), AnnotationEditErrorV1> {
        if self.revision != expected {
            return Err(AnnotationEditErrorV1::Stale);
        }
        let before = self.document.clone();
        let mut candidate = before.clone();
        apply(&mut candidate)?;
        validate_annotation_document_v1(&candidate).map_err(|_| AnnotationEditErrorV1::Invalid)?;
        for annotation in &candidate.annotations {
            let layer = layers
                .layers
                .iter()
                .find(|layer| layer.id == annotation.layer)
                .filter(|layer| {
                    layer.content_kind == LayerContentKindV1::Annotation && !layer.locked
                })
                .ok_or(AnnotationEditErrorV1::LayerUnavailable)?;
            let _ = layer;
            if let AnnotationAnchorV1::Vertex { vertex, .. } = annotation.anchor
                && !pattern.vertices.iter().any(|item| item.id == vertex)
            {
                return Err(AnnotationEditErrorV1::AnchorVertexMissing);
            }
        }
        self.document = candidate;
        self.commit(before);
        Ok(())
    }

    fn commit(&mut self, before: AnnotationDocumentV1) {
        self.undo.push(before);
        self.redo.clear();
        self.revision += 1;
    }
}
