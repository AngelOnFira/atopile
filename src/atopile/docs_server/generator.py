"""HTML documentation generator for Atopile projects."""

import json
import logging
from pathlib import Path
from typing import Dict, List, Any

from jinja2 import Environment, FileSystemLoader

from atopile import front_end
from atopile.datatypes import TypeRef

logger = logging.getLogger(__name__)


class DocGenerator:
    """Generate HTML documentation from Atopile modules."""
    
    def __init__(self, project_path: Path, output_dir: Path):
        self.project_path = project_path
        self.output_dir = output_dir
        self.modules = []
        self.files = []
        
        # Setup Jinja2
        template_dir = Path(__file__).parent / "templates"
        self.env = Environment(loader=FileSystemLoader(template_dir))
        
    def generate(self):
        """Generate complete documentation."""
        logger.info(f"Generating documentation for {self.project_path}")
        
        # Create output directory
        self.output_dir.mkdir(parents=True, exist_ok=True)
        static_dir = self.output_dir / "static"
        static_dir.mkdir(exist_ok=True)
        
        # Copy static files
        self._copy_static_files()
        
        # Scan for .ato files
        self._scan_project()
        
        # Generate pages
        self._generate_index()
        self._generate_module_pages()
        
        logger.info(f"Documentation generated in {self.output_dir}")
        
    def _copy_static_files(self):
        """Copy CSS and other static files."""
        import shutil
        static_src = Path(__file__).parent / "templates" / "static"
        static_dst = self.output_dir / "static"
        
        for file in static_src.glob("*"):
            shutil.copy2(file, static_dst / file.name)
            
    def _scan_project(self):
        """Scan project for .ato files and extract modules."""
        ato_files = list(self.project_path.rglob("*.ato"))
        
        for ato_file in ato_files:
            relative_path = ato_file.relative_to(self.project_path)
            
            # Try to parse the file
            try:
                modules_dict = front_end.bob.try_build_all_from_file(ato_file)
                
                file_info = {
                    'path': str(relative_path),
                    'module_count': len(modules_dict)
                }
                self.files.append(file_info)
                
                # Extract module information
                for ref, node in modules_dict.items():
                    module_data = self._extract_module_data(ato_file, ref, node)
                    module_data['file_path'] = str(relative_path)
                    module_data['path'] = f"{relative_path.stem}/{ref}"
                    self.modules.append(module_data)
                    
            except Exception as e:
                logger.warning(f"Failed to parse {relative_path}: {e}")
                # Fall back to basic parsing
                self._extract_basic_modules(ato_file, relative_path)
                
    def _extract_module_data(self, file_path: Path, ref: TypeRef, node) -> Dict[str, Any]:
        """Extract module data for documentation."""
        # Basic info
        data = {
            'name': str(ref),
            'type': type(node).__name__.replace('_driver', '').title(),
            'icon': self._get_icon_for_type(type(node).__name__),
            'docstring': None,
            'pins': [],
            'signals': [],
            'parameters': [],
            'instances': [],
            'connections': [],
            'assertions': [],
            'source_code': None
        }
        
        # Try to get source code snippet
        try:
            with open(file_path, 'r') as f:
                content = f.read()
                # Simple extraction - can be improved
                lines = content.split('\n')
                for i, line in enumerate(lines):
                    if f"module {ref}" in line or f"interface {ref}" in line:
                        # Get the module definition
                        start = i
                        indent = len(line) - len(line.lstrip())
                        source_lines = [line]
                        
                        for j in range(i + 1, len(lines)):
                            next_line = lines[j]
                            if next_line.strip() and len(next_line) - len(next_line.lstrip()) <= indent:
                                break
                            source_lines.append(next_line)
                            
                        data['source_code'] = '\n'.join(source_lines[:20])  # Limit to 20 lines
                        break
        except Exception:
            pass
            
        return data
        
    def _extract_basic_modules(self, file_path: Path, relative_path: Path):
        """Extract basic module info when full parsing fails."""
        try:
            with open(file_path, 'r') as f:
                content = f.read()
                
            lines = content.split('\n')
            
            for i, line in enumerate(lines):
                stripped = line.strip()
                if stripped.startswith(('module ', 'interface ', 'component ')):
                    parts = stripped.split()
                    if len(parts) >= 2:
                        block_type = parts[0]
                        name = parts[1].rstrip(':')
                        
                        # Look for docstring
                        docstring = None
                        if i + 1 < len(lines):
                            next_line = lines[i + 1].strip()
                            if next_line.startswith(('"""', "'''")):
                                docstring = next_line.strip('"\'')
                        
                        module_data = {
                            'name': name,
                            'type': block_type.title(),
                            'icon': self._get_icon_for_type(block_type),
                            'file_path': str(relative_path),
                            'path': f"{relative_path.stem}/{name}",
                            'docstring': docstring,
                            'pins': [],
                            'signals': [],
                            'parameters': [],
                            'instances': [],
                            'connections': [],
                            'assertions': [],
                            'source_code': None
                        }
                        
                        self.modules.append(module_data)
                        
        except Exception as e:
            logger.error(f"Failed to extract basic modules from {relative_path}: {e}")
            
    def _get_icon_for_type(self, type_name: str) -> str:
        """Get emoji icon for module type."""
        icons = {
            'module': '🔧',
            'interface': '🔌',
            'component': '📦',
            'Module': '🔧',
            'Interface': '🔌',
            'Component': '📦',
            'App': '📱'
        }
        return icons.get(type_name, '📄')
        
    def _generate_index(self):
        """Generate the index page."""
        template = self.env.get_template('index.html')
        
        # Calculate statistics
        stats = {
            'total_modules': sum(1 for m in self.modules if m['type'].lower() == 'module'),
            'total_interfaces': sum(1 for m in self.modules if m['type'].lower() == 'interface'),
            'total_components': sum(1 for m in self.modules if m['type'].lower() == 'component'),
            'total_files': len(self.files)
        }
        
        html = template.render(
            project_name=self.project_path.name,
            modules=self.modules,
            all_modules=sorted(self.modules, key=lambda x: x['name']),
            files=sorted(self.files, key=lambda x: x['path']),
            stats=stats
        )
        
        (self.output_dir / 'index.html').write_text(html)
        
    def _generate_module_pages(self):
        """Generate individual module pages."""
        template = self.env.get_template('module.html')
        module_dir = self.output_dir / 'module'
        
        for module in self.modules:
            # Create module directory structure
            module_path = module_dir / module['path']
            module_path.parent.mkdir(parents=True, exist_ok=True)
            
            html = template.render(
                project_name=self.project_path.name,
                module=module,
                modules=self.modules,
                current_module=module['path']
            )
            
            (module_path.with_suffix('.html')).write_text(html)