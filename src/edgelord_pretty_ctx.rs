use comrade_lisp::diagnostics::pretty::{PrettyCtx, PrettyPrinter, PrettyDialect, PrettyLimits, PrinterRegistry, PrinterKey};
use comrade_lisp::diagnostics::DiagnosticContext;
use comrade_lisp::proof_state::ProofState;
use tower_lsp::lsp_types::Url;

/// Ephemeral pretty-printing context for LSP requests.
///
/// **Lifetime**: Created per hover/inlay/diagnostic request, discarded after.
/// **Design**: Borrowed wrapper avoiding copies of large objects.
pub struct EdgeLordPrettyCtx<'a> {
    registry: &'a PrinterRegistry,
    printer: &'a dyn PrettyPrinter,  // resolved once
    dialect: PrettyDialect,
    limits: PrettyLimits,
    files: &'a DiagnosticContext<'a>,
    proof: &'a ProofState,
    document_uri: &'a Url,
}

impl<'a> EdgeLordPrettyCtx<'a> {
    pub fn new(
        registry: &'a PrinterRegistry,
        dialect: PrettyDialect,
        limits: PrettyLimits,
        proof: &'a ProofState,
        files: &'a DiagnosticContext<'a>,
        document_uri: &'a Url,
    ) -> Self {
        let key = PrinterKey {
            doctrine: None,  // Use global default; future: extract from proof
            dialect,
        };
        let printer = registry.resolve(key);
        
        Self {
            registry,
            printer,
            dialect,
            limits,
            files,
            proof,
            document_uri,
        }
    }

    pub fn document_uri(&self) -> &'a Url {
        self.document_uri
    }
}

impl<'a> PrettyCtx for EdgeLordPrettyCtx<'a> {
    fn printer(&self) -> &dyn PrettyPrinter {
        self.printer
    }
    
    fn proof(&self) -> &ProofState {
        self.proof
    }
    
    fn files(&self) -> &DiagnosticContext<'_> {
        self.files
    }
    
    fn limits(&self) -> PrettyLimits {
        self.limits
    }
}
