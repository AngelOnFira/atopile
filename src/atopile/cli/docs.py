"""CLI command definition for `ato docs`."""

import logging
from pathlib import Path
from typing import Annotated

import typer

from atopile.docs_server.generator import DocGenerator
from atopile.docs_server.server import DocServer
from atopile.config import config

logger = logging.getLogger(__name__)


def docs(
    port: Annotated[
        int,
        typer.Option("--port", "-p", help="Port for web server")
    ] = 8080,
    no_browser: Annotated[
        bool,
        typer.Option("--no-browser", help="Don't open browser automatically")
    ] = False,
    output_dir: Annotated[
        str | None,
        typer.Option("--output", "-o", help="Output directory for generated docs")
    ] = None,
    build_only: Annotated[
        bool,
        typer.Option("--build-only", help="Only generate docs, don't serve")
    ] = False,
):
    """
    Generate and serve web documentation for all modules in the project.
    
    Creates beautiful HTML documentation similar to Rust's cargo doc.
    """
    try:
        # Get project path
        try:
            config.apply_options(entry=None)
            project_path = Path.cwd()
        except Exception:
            # No project config, use current directory
            project_path = Path.cwd()
            
        # Set output directory
        if output_dir:
            docs_dir = Path(output_dir)
        else:
            docs_dir = project_path / ".docs" / "html"
            
        typer.echo(f"📚 Generating documentation...")
        typer.echo(f"Project: {project_path}")
        typer.echo(f"Output: {docs_dir}")
        
        # Generate documentation
        generator = DocGenerator(project_path, docs_dir)
        generator.generate()
        
        typer.echo(f"✅ Documentation generated successfully!")
        
        # Serve documentation unless build-only
        if not build_only:
            server = DocServer(docs_dir, port)
            server.serve(open_browser=not no_browser)
        else:
            typer.echo(f"📁 Documentation saved to: {docs_dir}")
            typer.echo(f"💡 To view, run: python -m http.server -d {docs_dir}")
        
    except Exception as e:
        logger.error(f"Failed to generate documentation: {e}")
        typer.echo(f"❌ Error: {e}", err=True)
        raise typer.Exit(1)